import { forwardRef, HttpException, HttpStatus, Inject, Injectable } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'
import { uuidv7 } from 'uuidv7'

import { arrayIsConsistent } from '@/common/utils/arrayIsConsistent'
import { getISOWithMicrosecond } from '@/common/utils/getISOWithMicrosecond'
import { isArray } from '@/common/utils/isArray'
import { isNumeric } from '@/common/utils/isNumeric'
import { isObject } from '@/common/utils/isObject'
import { isPrimitiveArray } from '@/common/utils/isPrimitiveArray'
import { pickPrimitives } from '@/common/utils/pickPrimitives'
import { toBoolean } from '@/common/utils/toBolean'
import { AiService } from '@/core/ai/ai.service'
import { InlineVectorEntryDto } from '@/core/ai/dto/inline-vector-entry.dto'
import { BILLING_POLICY_PORT, BillingPolicyPort } from '@/core/billing-policy/billing-policy.port'
import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_LABEL,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_RELATION_DEFAULT
} from '@/core/common/constants'
import { suggestPropertyType } from '@/core/common/normalizeRecord'
import { MaybeArray } from '@/core/common/types'
import { CreateEntityDto } from '@/core/entity/dto/create-entity.dto'
import { EntityQueryService } from '@/core/entity/entity-query.service'
import { TEntityPropertiesNormalized } from '@/core/entity/entity.types'
import { ImportJsonDto } from '@/core/entity/import-export/dto/import-json.dto'
import {
  TImportOptions,
  TImportJsonPayload,
  TImportRecordsRelation,
  TImportSummary,
  WithId,
  TImportQueue
} from '@/core/entity/import-export/import.types'
import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { PropertyDto } from '@/core/property/dto/property.dto'
import { PROPERTY_TYPE_NUMBER, PROPERTY_TYPE_STRING } from '@/core/property/property.constants'
import { PropertyService } from '@/core/property/property.service'
import { TPropertyPrimitiveValue } from '@/core/property/property.types'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'

type VectorDraft = {
  draftId: string
  label: string
  vectors: InlineVectorEntryDto[]
}

@Injectable()
export class ImportService {
  constructor(
    private readonly entityQueryService: EntityQueryService,
    private readonly kuEventsService: KuEventsService,
    @Inject(BILLING_POLICY_PORT)
    private readonly billingPolicyService: BillingPolicyPort,

    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService,

    @Inject(forwardRef(() => PropertyService))
    private readonly propertyService: PropertyService,

    private readonly aiService: AiService
  ) {}

  getValueParameters(value: MaybeArray<TPropertyPrimitiveValue>) {
    if (Array.isArray(value)) {
      const isInconsistentArray = !arrayIsConsistent(value)
      const isEmptyArray = !value.length || value.every((v) => typeof v === 'undefined')

      return {
        isInconsistentArray,
        isEmptyArray
      }
    }

    return {}
  }

  parsePrimitive({
    key,
    value,
    target,
    options
  }: {
    key: string
    value: MaybeArray<TPropertyPrimitiveValue>
    target: WithId<CreateEntityDto>
    options: TImportOptions
  }) {
    if (key === RUSHDB_KEY_ID) {
      target.id = value as string
    } else if (key === RUSHDB_KEY_LABEL) {
      target.label = value as string
    } else if (key === RUSHDB_KEY_PROPERTIES_META) {
      // @TODO: Use it for schema validation https://github.com/rush-db/rushdb/issues/43
    } else if (key === RUSHDB_KEY_PROJECT_ID) {
      // do nothing
    } else if (value === null || value === undefined) {
      // null/undefined means the field is unset — never create a property for it.
    } else if (options.skipEmptyValues && value === '') {
      // skipEmptyValues: an empty string is treated as unset — no property created.
      // (0 and false are real values and are handled in the branch below.)
    } else {
      const property = {
        id: uuidv7(),
        name: key,
        created: getISOWithMicrosecond(),
        value: '',
        type: PROPERTY_TYPE_STRING
      } as PropertyDto & { created: string }

      if (isArray(value)) {
        // Strip null/undefined elements (and, with skipEmptyValues, '' elements);
        // an array that is entirely null/empty collapses to "unset".
        const cleaned = value.filter(
          (item) => item !== null && item !== undefined && !(options.skipEmptyValues && item === '')
        )
        if (value.length > 0 && cleaned.length === 0) {
          return
        }
        // with skipEmptyValues, a genuinely empty [] is treated as unset too
        if (options.skipEmptyValues && cleaned.length === 0) {
          return
        }
        if (options.suggestTypes) {
          const { isEmptyArray, isInconsistentArray } = this.getValueParameters(cleaned)
          if (isEmptyArray) {
            // Store a genuine empty array. (Legacy records may hold the
            // RUSHDB_VALUE_EMPTY_ARRAY sentinel string instead; that is converted back to
            // [] on read by the data interceptor and cleaned up by the `migrate-empty-arrays` CLI.)
            property.value = []
          } else if (options.convertNumericValuesToNumbers && cleaned.every(isNumeric)) {
            property.value = cleaned.map(Number)
            property.type = PROPERTY_TYPE_NUMBER
          } else if (isInconsistentArray && !cleaned.every(isNumeric)) {
            property.value = cleaned.map(String)
            property.type = PROPERTY_TYPE_STRING
          } else {
            property.value = cleaned
            property.type = suggestPropertyType(cleaned[0])
          }
        } else {
          property.value = cleaned.map(String)
          property.type = PROPERTY_TYPE_STRING
        }
      } else {
        if (options.suggestTypes) {
          if (options.convertNumericValuesToNumbers && isNumeric(value)) {
            //
            property.value = Number(value)
            property.type = PROPERTY_TYPE_NUMBER
          } else {
            property.value = value
            property.type = suggestPropertyType(value)
          }
        } else {
          property.value = String(value)
          property.type = PROPERTY_TYPE_STRING
        }
      }

      target.properties?.push(property)
    }
  }

  serializeBFS(
    data: TImportJsonPayload,
    label: string,
    options: TImportOptions
  ): [Array<WithId<CreateEntityDto>>, TImportRecordsRelation[], VectorDraft[]] {
    const entities: Array<WithId<CreateEntityDto>> = []
    const relations: TImportRecordsRelation[] = []
    const vectorDrafts: VectorDraft[] = []

    const queue: Array<TImportQueue> = []

    if (isArray(data) && data.length > 0) {
      data.forEach((value: WithId<CreateEntityDto>) =>
        queue.push({
          ...options,
          key: options.capitalizeLabels ? label?.toUpperCase() : label,
          value,
          target: null
        })
      )
    } else {
      // Determine whether to skip creating the root node.
      // If no explicit label provided, always skip the root to avoid creating an empty-labeled node.
      // If label is provided, create root only when meaningful (has primitives or multiple complex children).
      const hasExplicitLabel = toBoolean((label ?? '').trim())
      const hasPrimitiveAtRoot = toBoolean(Object.keys(pickPrimitives(data)).length)
      let complexChildrenCount = 0
      if (isObject(data)) {
        for (const [, v] of Object.entries(data)) {
          if (isObject(v) || (isArray(v) && !isPrimitiveArray(v))) {
            complexChildrenCount += 1
          }
        }
      }
      const skip = !hasExplicitLabel || !(hasPrimitiveAtRoot || complexChildrenCount > 1)
      queue.push({
        ...options,
        key: options.capitalizeLabels ? label?.toUpperCase() : label,
        value: data,
        target: null,
        // @FYI: Skip creation redundant start Record with no meaningful data:
        // { someObject: {...} } previously led to creation of two records instead of one =>
        // object (node:NULL) that holds other object (node:someObject)
        skip
      })
    }

    const parse = ({
      value: valuePart,
      target,
      parentId
    }: {
      value: TImportJsonPayload
      target: WithId<CreateEntityDto>
      parentId?: string
    }) => {
      Object.entries(valuePart).forEach(([key, value]) => {
        if (isObject(value)) {
          queue.push({
            ...options,
            key: options.capitalizeLabels ? key?.toUpperCase() : key,
            value,
            parentId,
            target
          })
        } else if (isArray(value) && !isPrimitiveArray(value)) {
          value.forEach((val: WithId<CreateEntityDto>) =>
            queue.push({
              ...options,
              key: options.capitalizeLabels ? key?.toUpperCase() : key,
              value: val,
              parentId,
              target
            })
          )
        } else {
          this.parsePrimitive({ key, value, target, options })
        }
      })
    }

    while (queue.length > 0) {
      const current = queue.shift()
      const { key, value, parentId, target } = current
      const recordDraft: WithId<CreateEntityDto> = {
        id: uuidv7(),
        properties: [],
        label: options.capitalizeLabels ? key?.toUpperCase() : key
      } as WithId<CreateEntityDto>

      // Extract inline vectors before BFS parse to prevent $vectors from being treated as child entities
      const rawVectors: InlineVectorEntryDto[] | undefined = (value as any)?.$vectors
      if (rawVectors !== undefined) {
        delete (value as any).$vectors
      }

      // If we do not skip, create the entity and relation from parent (if parent exists)
      if (!toBoolean(current?.skip)) {
        if (toBoolean(parentId)) {
          relations.push({
            source: parentId,
            target: recordDraft.id,
            type: options.relationshipType?.trim() || RUSHDB_RELATION_DEFAULT
          })
        }
        entities.push(recordDraft)

        if (Array.isArray(rawVectors) && rawVectors.length > 0) {
          vectorDrafts.push({ draftId: recordDraft.id, label: recordDraft.label, vectors: rawVectors })
        }
      }

      // Determine the parent id to pass to children:
      // - If we skipped current, bubble up the existing parentId so children become siblings at that level
      // - If not skipped, current recordDraft becomes the parent for its children
      const childParentId = toBoolean(current?.skip) ? parentId : recordDraft.id

      parse({ value, target: recordDraft, parentId: childParentId })
    }

    return [entities, relations, vectorDrafts]
  }

  async checkLimits(recordsCount: number, projectId: string, transaction: Transaction) {
    const workspaceInstance = await this.workspaceService.getWorkspaceByProject(projectId, transaction)
    const workspaceId = workspaceInstance?.id

    // Estimate KU for the import (conservative: assume 10 properties per record average)
    const estimatedKu = recordsCount * 10

    await this.billingPolicyService.assertProjectOperationAllowed(workspaceId, { estimatedKu })

    return true
  }

  async importRecords(
    {
      data,
      label,
      options = {
        suggestTypes: true,
        returnResult: false,
        relationshipType: RUSHDB_RELATION_DEFAULT
      }
    }: ImportJsonDto,
    projectId: string,
    transaction: Transaction,
    customTransaction: Transaction = transaction
  ): Promise<boolean | TImportSummary | TEntityPropertiesNormalized[]> {
    if (typeof data === 'string' || !data || typeof data !== 'object') {
      throw new HttpException('Import data must be a JSON object or array', HttpStatus.BAD_REQUEST)
    }

    // @FYI: Approximate time for 25MB JSON: 2.5s. RUST WASM?))))))
    const [records, relations, vectorDrafts] = this.serializeBFS(data as TImportJsonPayload, label, options)

    // Will throw error if the amount of uploading Records is more than allowed by current plan
    await this.checkLimits(records.length, projectId, transaction)

    // Get workspace for billing attribution
    const workspace = await this.workspaceService.getWorkspaceByProject(projectId, transaction)
    const workspaceId = workspace?.id

    const CHUNK_SIZE = 1000
    const shouldAccumulateResult = options.returnResult === true && records.length <= CHUNK_SIZE

    let result = []
    // Map draft record ids (generated during serialization) to actual persisted record ids after upsert/create
    const draftToPersistedId = new Map<string, string>()
    for (let i = 0; i < records.length; i += CHUNK_SIZE) {
      const recordsChunk = records.slice(i, i + CHUNK_SIZE)

      const data = await this.processRecordsChunk({
        transaction: customTransaction,
        options: {
          ...options,
          returnResult: shouldAccumulateResult
        },
        recordsChunk,
        projectId
      })

      // Extract id map and results (if requested)
      const idmap = data.records?.[0]?.get('idmap') ?? []
      if (Array.isArray(idmap)) {
        for (const item of idmap) {
          if (item && item.draftId && item.persistedId) {
            draftToPersistedId.set(item.draftId, item.persistedId)
          }
        }
      }

      if (shouldAccumulateResult) {
        const chunkData = data.records?.[0]?.get('data')
        if (Array.isArray(chunkData)) {
          result = result.concat(chunkData)
        }
      }
    }

    // Remap relations to persisted IDs in case upsert matched existing records
    const remappedRelations = relations.map((rel) => ({
      source: draftToPersistedId.get(rel.source) ?? rel.source,
      target: draftToPersistedId.get(rel.target) ?? rel.target,
      type: rel.type,
      properties: rel.properties
    }))

    // Write inline vectors now that all record IDs are known
    for (const vd of vectorDrafts) {
      const persistedId = draftToPersistedId.get(vd.draftId) ?? vd.draftId
      await this.aiService.resolveAndWriteInlineVectors(
        projectId,
        vd.label,
        persistedId,
        vd.vectors,
        customTransaction
      )
    }

    // Emit bulk entity creation event (one coalesced event per import, not per chunk)
    if (records.length > 0) {
      let totalPropertyCount = 0
      for (const record of records) {
        totalPropertyCount += (record.properties ?? []).length
      }

      this.kuEventsService.emitBulk(workspaceId, projectId, KuOperation.ENTITY_CREATED, records.length, {
        propertyCount: totalPropertyCount
      })
    }

    for (let i = 0; i < remappedRelations.length; i += CHUNK_SIZE) {
      const relationsChunk = remappedRelations.slice(i, i + CHUNK_SIZE)
      await this.processRelationshipsChunk({
        relationsChunk,
        projectId,
        transaction: customTransaction
      })
    }

    // Emit bulk relationship creation event
    if (remappedRelations.length > 0) {
      this.kuEventsService.emitBulk(
        workspaceId,
        projectId,
        KuOperation.RELATIONSHIP_CREATED,
        remappedRelations.length
      )
    }

    const upsertRequested = toBoolean(options.mergeStrategy) || isArray(options.mergeBy)

    if (upsertRequested) {
      await this.propertyService.deleteOrphanProps({
        projectId,
        transaction
      })
    }

    if (options.returnResult && !shouldAccumulateResult) {
      return {
        message: `Import complete. ${records.length} records processed. Result omitted for large imports — use /records/search to retrieve records.`,
        count: records.length
      }
    }

    return shouldAccumulateResult ? result : true
  }

  async processRecordsChunk({
    recordsChunk,
    projectId,
    transaction,
    options
  }: {
    projectId: string
    recordsChunk: WithId<CreateEntityDto>[]
    transaction: Transaction
    options: TImportOptions
  }) {
    // Upsert path if mergeStrategy or mergeBy provided (mergeStrategy defaults to 'append' behavior when omitted).
    const upsertRequested = typeof options.mergeStrategy !== 'undefined' || Array.isArray(options.mergeBy)
    if (upsertRequested) {
      const rewrite = options.mergeStrategy === 'rewrite'
      return transaction.run(
        this.entityQueryService.importUpsertRecords({
          withResults: options.returnResult,
          rewrite
        }),
        {
          records: recordsChunk,
          projectId,
          mergeBy: options.mergeBy,
          rewrite
        }
      )
    }

    return transaction.run(this.entityQueryService.importRecords(options.returnResult), {
      records: recordsChunk,
      projectId
    })
  }

  async processRelationshipsChunk({
    relationsChunk,
    transaction,
    projectId
  }: {
    projectId: string
    relationsChunk: TImportRecordsRelation[]
    transaction: Transaction
  }) {
    await transaction.run(this.entityQueryService.linkRecords(), {
      relations: relationsChunk,
      projectId
    })
  }
}
