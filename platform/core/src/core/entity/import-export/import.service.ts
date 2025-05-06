import { forwardRef, HttpException, HttpStatus, Inject, Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'
import { QueryRunner } from 'neogma'
import { uuidv7 } from 'uuidv7'

import { arrayIsConsistent } from '@/common/utils/arrayIsConsistent'
import { getISOWithMicrosecond } from '@/common/utils/getISOWithMicrosecond'
import { isArray } from '@/common/utils/isArray'
import { isNumeric } from '@/common/utils/isNumeric'
import { isObject } from '@/common/utils/isObject'
import { isPrimitiveArray } from '@/common/utils/isPrimitiveArray'
import { pickPrimitives } from '@/common/utils/pickPrimitives'
import { toBoolean } from '@/common/utils/toBolean'
import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_LABEL,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_RELATION_DEFAULT,
  RUSHDB_VALUE_EMPTY_ARRAY,
  RUSHDB_VALUE_NULL
} from '@/core/common/constants'
import { suggestPropertyType } from '@/core/common/normalizeRecord'
import { MaybeArray } from '@/core/common/types'
import { CreateEntityDto } from '@/core/entity/dto/create-entity.dto'
import { EntityQueryService } from '@/core/entity/entity-query.service'
import { ImportJsonDto } from '@/core/entity/import-export/dto/import-json.dto'
import {
  TImportOptions,
  TImportJsonPayload,
  TImportRecordsRelation,
  WithId,
  TImportQueue
} from '@/core/entity/import-export/import.types'
import { TEntityPropertiesNormalized } from '@/core/entity/model/entity.interface'
import { PropertyDto } from '@/core/property/dto/property.dto'
import {
  PROPERTY_TYPE_NULL,
  PROPERTY_TYPE_NUMBER,
  PROPERTY_TYPE_STRING,
  PROPERTY_TYPE_VECTOR
} from '@/core/property/property.constants'
import { TPropertyPrimitiveValue } from '@/core/property/property.types'
import { TWorkspaceLimits } from '@/dashboard/workspace/model/workspace.interface'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
export class ImportService {
  constructor(
    private readonly configService: ConfigService,
    private readonly neogmaService: NeogmaService,
    private readonly entityQueryService: EntityQueryService,

    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService
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
    const valueParameters = this.getValueParameters(value)

    if (key === RUSHDB_KEY_ID) {
      target.id = value as string
    } else if (key === RUSHDB_KEY_LABEL) {
      target.label = value as string
    } else if (key === RUSHDB_KEY_PROPERTIES_META) {
      // @TODO: Use it for schema validation https://github.com/rush-db/rushdb/issues/43
    } else if (key === RUSHDB_KEY_PROJECT_ID) {
      // do nothing
    } else {
      const property = {
        id: uuidv7(),
        name: key,
        created: getISOWithMicrosecond(),
        value: '',
        type: PROPERTY_TYPE_STRING
      } as PropertyDto & { created: string }

      if (isArray(value)) {
        if (options.suggestTypes) {
          const { isEmptyArray, isInconsistentArray } = valueParameters
          if (isEmptyArray) {
            property.value = RUSHDB_VALUE_EMPTY_ARRAY
          } else if (options.castNumberArraysToVectors && value.every(isNumeric)) {
            property.value = value.map(Number)
            property.type = PROPERTY_TYPE_VECTOR
          } else if (options.convertNumericValuesToNumbers && value.every(isNumeric)) {
            property.value = value.map(Number)
            property.type = options.castNumberArraysToVectors ? PROPERTY_TYPE_VECTOR : PROPERTY_TYPE_NUMBER
          } else if (isInconsistentArray && !options.castNumberArraysToVectors && !value.every(isNumeric)) {
            property.value = value.map(String)
            property.type = PROPERTY_TYPE_STRING
          } else if (value[0] === null) {
            property.value = value.map(() => RUSHDB_VALUE_NULL)
            property.type = PROPERTY_TYPE_NULL
          } else {
            property.value = value
            property.type = suggestPropertyType(value[0])
          }
        } else {
          property.value = value.map(String)
          property.type = PROPERTY_TYPE_STRING
        }
      } else {
        if (options.suggestTypes) {
          if (options.convertNumericValuesToNumbers && isNumeric(value)) {
            //
            property.value = Number(value)
            property.type = PROPERTY_TYPE_NUMBER
          } else {
            const valueType = suggestPropertyType(value)

            property.value = valueType === PROPERTY_TYPE_NULL ? RUSHDB_VALUE_NULL : value
            property.type = valueType
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
    payload: TImportJsonPayload,
    label: string,
    options: TImportOptions
  ): [Array<WithId<CreateEntityDto>>, TImportRecordsRelation[]] {
    const entities: Array<WithId<CreateEntityDto>> = []
    const relations: TImportRecordsRelation[] = []

    const queue: Array<TImportQueue> = []

    if (isArray(payload) && payload.length > 0) {
      payload.forEach((value: WithId<CreateEntityDto>) =>
        queue.push({
          ...options,
          key: options.capitalizeLabels ? label.toUpperCase() : label,
          value,
          target: null
        })
      )
    } else {
      const skip = !toBoolean(Object.keys(pickPrimitives(payload)).length)
      queue.push({
        ...options,
        key: options.capitalizeLabels ? label.toUpperCase() : label,
        value: payload,
        target: null,
        // @FYI: Skip creation redundant start Record with no meaningful data:
        // { someObject: {...} } previously led to creation of two records instead of one =>
        // object (node:NULL) that holds other object (node:someObject)
        skip
      })
    }

    const parse = ({
      value: valuePart,
      target
    }: {
      value: TImportJsonPayload
      target: WithId<CreateEntityDto>
    }) => {
      Object.entries(valuePart).forEach(([key, value]) => {
        if (isObject(value)) {
          queue.push({
            ...options,
            key: options.capitalizeLabels ? key.toUpperCase() : key,
            value,
            parentId: target.id,
            target
          })
        } else if (isArray(value) && !isPrimitiveArray(value)) {
          value.forEach((val: WithId<CreateEntityDto>) =>
            queue.push({
              ...options,
              key: options.capitalizeLabels ? key.toUpperCase() : key,
              value: val,
              parentId: target.id,
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
        label: options.capitalizeLabels ? key.toUpperCase() : key
      } as WithId<CreateEntityDto>

      if (!toBoolean(current?.skip)) {
        relations.push({
          source: parentId,
          target: recordDraft.id,
          type: options.relationshipType?.trim() || RUSHDB_RELATION_DEFAULT
        })
        entities.push(recordDraft)
      }

      parse({ value, target: recordDraft })
    }

    return [entities, relations]
  }

  async checkLimits(recordsCount: number, projectId: string, transaction: Transaction) {
    if (toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
      return true
    }

    // @FYI: This exists to prevent saving more Records than allowed by current plan
    const workspaceInstance = await this.workspaceService.getWorkspaceByProject(projectId, transaction)
    const workspaceStats = await this.workspaceService.getAccumulatedWorkspaceStats(
      workspaceInstance,
      transaction
    )

    const limits = JSON.parse(workspaceInstance.dataValues.limits) as TWorkspaceLimits

    if (workspaceStats.records + recordsCount > limits.records) {
      throw new HttpException(
        'The number of items you are trying to send exceeds your limits.',
        HttpStatus.PAYMENT_REQUIRED
      )
    }
  }

  async importRecords(
    {
      payload,
      label,
      options = {
        suggestTypes: true,
        returnResult: false,
        relationshipType: RUSHDB_RELATION_DEFAULT
      }
    }: ImportJsonDto,
    projectId: string,
    transaction: Transaction,
    queryRunner?: QueryRunner
  ): Promise<boolean | TEntityPropertiesNormalized[]> {
    const runner = queryRunner || this.neogmaService.createRunner()

    // @FYI: Approximate time for 25MB JSON: 2.5s. RUST WASM?))))))
    const [records, relations] = this.serializeBFS(payload, label, options)

    // Will throw error if the amount of uploading Records is more than allowed by current plan
    await this.checkLimits(records.length, projectId, transaction)

    const CHUNK_SIZE = 1000

    // @TODO: Accumulate result only if records <= 1000. Otherwise - ignore options.returnResult
    let result = []
    for (let i = 0; i < records.length; i += CHUNK_SIZE) {
      const recordsChunk = records.slice(i, i + CHUNK_SIZE)

      const data = await this.processRecordsChunk({
        transaction,
        options,
        recordsChunk,
        projectId,
        queryRunner: runner
      })

      if (options.returnResult) {
        result = result.concat(data.records?.[0]?.get('data'))
      }
    }

    for (let i = 0; i < relations.length; i += CHUNK_SIZE) {
      const relationsChunk = relations.slice(i, i + CHUNK_SIZE)
      await this.processRelationshipsChunk({ relationsChunk, transaction, projectId, queryRunner: runner })
    }

    return options.returnResult ? result : true
  }

  async processRecordsChunk({
    recordsChunk,
    projectId,
    transaction,
    queryRunner,
    options
  }: {
    projectId: string
    recordsChunk: WithId<CreateEntityDto>[]
    transaction: Transaction
    queryRunner?: QueryRunner
    options: TImportOptions
  }) {
    const runner = queryRunner || this.neogmaService.createRunner()

    return await runner.run(
      this.entityQueryService.importRecords(options.returnResult),
      {
        records: recordsChunk,
        projectId
      },
      transaction
    )
  }

  async processRelationshipsChunk({
    relationsChunk,
    transaction,
    projectId,
    queryRunner
  }: {
    projectId: string
    relationsChunk: TImportRecordsRelation[]
    transaction: Transaction
    queryRunner?: QueryRunner
  }) {
    const runner = queryRunner || this.neogmaService.createRunner()

    await runner.run(
      this.entityQueryService.linkRecords(),
      {
        relations: relationsChunk,
        projectId
      },
      transaction
    )
  }
}
