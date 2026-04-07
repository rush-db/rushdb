import {
  ConflictException,
  HttpException,
  HttpStatus,
  Inject,
  Injectable,
  Logger,
  NotFoundException,
  UnprocessableEntityException
} from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { int as neo4jInt, Transaction } from 'neo4j-driver'

import { isDevMode } from '@/common/utils/isDevMode'
import { AiQueryService } from '@/core/ai/ai-query.service'
import { EmbeddingIndexRepository } from '@/core/ai/embedding-index.repository'
import {
  buildVectorIndexName,
  buildVectorPropertyName,
  isValidEmbeddingDimensions,
  type EmbeddingIndexSimilarityFunction,
  type EmbeddingIndexSourceType
} from '@/core/ai/embedding-index.utils'
import { EmbeddingProviderService } from '@/core/ai/embedding-provider.service'
import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { parseWhereClause } from '@/core/search/parser/buildQuery'
import { NeogmaService } from '@/database/neogma/neogma.service'

import { randomUUID } from 'crypto'

import type { OntologyItem, OntologyProperty, OntologyRelationship } from '@/core/ai/ai.types'
import type { CreateEmbeddingIndexDto } from '@/core/ai/dto/create-embedding-index.dto'
import type { InlineVectorEntryDto } from '@/core/ai/dto/inline-vector-entry.dto'
import type { SemanticSearchDto } from '@/core/ai/dto/semantic-search.dto'
import type { UpsertIndexVectorsDto } from '@/core/ai/dto/upsert-index-vectors.dto'
import type { EmbeddingIndexRow } from '@/database/sql/schema/types'

import { ProjectRepository } from '@/dashboard/project/model/project.repository'
import { BILLING_POLICY_PORT, BillingPolicyPort } from '@/core/billing-policy/billing-policy.port'
import { estimateEmbeddingBatchKu, estimateTokens } from '@/core/ai/embedding.utils'

/** How long (ms) a cached ontology is considered fresh before a recalculation is triggered. */
const ONTOLOGY_CACHE_TTL_MS = 60 * 60 * 1000 // 1 hour

@Injectable()
export class AiService {
  constructor(
    private readonly aiQueryService: AiQueryService,
    private readonly configService: ConfigService,
    private readonly kuEventsService: KuEventsService,
    @Inject(BILLING_POLICY_PORT)
    private readonly billingPolicyService: BillingPolicyPort,
    private readonly projectRepository: ProjectRepository,
    private readonly embeddingIndexRepository: EmbeddingIndexRepository,
    private readonly embeddingProviderService: EmbeddingProviderService,
    private readonly neogmaService: NeogmaService
  ) {}

  async getOntology({
    projectId,
    workspaceId,
    labels,
    transaction
  }: {
    projectId: string
    /** When provided, a COMPUTE_OPERATION KU event is emitted on cache miss / recalculation. */
    workspaceId?: string
    labels?: string[]
    transaction: Transaction
  }): Promise<OntologyItem[]> {
    const tTotal = Date.now()

    // ── 1. Check persisted cache in SQL projects table ────────────────────────
    const projectRow = await this.projectRepository.findById(projectId)
    const cachedAt: string | null = projectRow?.ontologyCachedAt ?? null
    const cacheJson: string | null = projectRow?.ontologyCache ?? null

    if (cachedAt && cacheJson) {
      const ageMs = Date.now() - new Date(cachedAt).getTime()
      if (ageMs < ONTOLOGY_CACHE_TTL_MS) {
        // Cache is fresh — deserialise, optionally filter, return immediately
        const fullOntology: OntologyItem[] = JSON.parse(cacheJson)
        const result =
          labels?.length ? fullOntology.filter((item) => labels.includes(item.label)) : fullOntology

        isDevMode(() =>
          Logger.debug(
            `[AiService] ontology: cache hit — age ${Math.round(ageMs / 1000)}s, ${result.length}/${fullOntology.length} labels, total ${Date.now() - tTotal}ms`
          )
        )
        return result
      }
    }

    // ── 2. Cache is stale/missing — recalculate the FULL ontology ──────────
    // Always run without a label filter so the persisted cache is complete.
    // Label filtering (if requested) is applied in-memory after building.
    const params: Record<string, any> = { projectId }

    const t0 = Date.now()

    // Run Q1, Q2, Q3 in parallel — all are read-only
    const [labelsResult, propertiesResult, relationshipsResult] = await Promise.all([
      transaction.run(this.aiQueryService.getLabelsQuery(), params),
      transaction.run(this.aiQueryService.getPropertiesWithValuesQuery(), params),
      transaction.run(this.aiQueryService.getRelationshipsQuery(), params)
    ])

    // ---------- Build label → count map (Q1) ----------
    const labelCountMap = new Map<string, number>()
    for (const r of labelsResult.records) {
      const label = r.get('label') as string
      const count = (r.get('recordCount') as any)?.toNumber?.() ?? Number(r.get('recordCount'))
      labelCountMap.set(label, count)
    }

    // ---------- Build label → properties map (Q2) ----------
    const labelPropsMap = new Map<string, OntologyProperty[]>()
    for (const r of propertiesResult.records) {
      const label = r.get('label') as string
      const prop: OntologyProperty = {
        id: r.get('propId') as string,
        name: r.get('propName') as string,
        type: r.get('propType') as string
      }

      const sampleValues = r.get('sampleValues')
      const minValue = r.get('minValue')
      const maxValue = r.get('maxValue')

      if (sampleValues != null) {
        prop.values = sampleValues
      }
      if (minValue != null) {
        prop.min = typeof minValue === 'object' && minValue?.toNumber ? minValue.toNumber() : minValue
      }
      if (maxValue != null) {
        prop.max = typeof maxValue === 'object' && maxValue?.toNumber ? maxValue.toNumber() : maxValue
      }

      if (!labelPropsMap.has(label)) {
        labelPropsMap.set(label, [])
      }
      labelPropsMap.get(label)!.push(prop)
    }

    // ---------- Build label → relationships map (Q3) ----------
    // Relationships are stored bidirectionally: fromLabel gets 'out', toLabel gets 'in'.
    const labelRelsMap = new Map<string, OntologyRelationship[]>()

    const addRel = (label: string, rel: OntologyRelationship) => {
      if (!labelRelsMap.has(label)) {
        labelRelsMap.set(label, [])
      }
      const existing = labelRelsMap.get(label)!
      const key = `${rel.direction}|${rel.label}|${rel.type}`
      if (!existing.some((e) => `${e.direction}|${e.label}|${e.type}` === key)) {
        existing.push(rel)
      }
    }

    for (const r of relationshipsResult.records) {
      const fromLabel = r.get('fromLabel') as string
      const relType = r.get('relType') as string
      const toLabel = r.get('toLabel') as string

      if (fromLabel === toLabel) {
        continue
      } // skip self-loops

      addRel(fromLabel, { label: toLabel, type: relType, direction: 'out' })
      addRel(toLabel, { label: fromLabel, type: relType, direction: 'in' })
    }

    // ---------- Merge into OntologyItem[] ----------
    // Include all labels from Q1 plus any additional labels seen in Q2/Q3
    const allLabels = new Set<string>([
      ...labelCountMap.keys(),
      ...labelPropsMap.keys(),
      ...labelRelsMap.keys()
    ])

    const ontology: OntologyItem[] = []

    for (const label of allLabels) {
      const rels = labelRelsMap.get(label) ?? []
      rels.sort((a, b) => {
        if (a.direction !== b.direction) {
          return a.direction < b.direction ? -1 : 1
        }
        if (a.label !== b.label) {
          return a.label.localeCompare(b.label)
        }
        return a.type.localeCompare(b.type)
      })

      ontology.push({
        label,
        count: labelCountMap.get(label) ?? 0,
        properties: labelPropsMap.get(label) ?? [],
        relationships: rels
      })
    }

    // Sort by count descending for consistent output
    ontology.sort((a, b) => b.count - a.count)

    const elapsed = Date.now() - t0

    isDevMode(() =>
      Logger.debug(
        `[AiService] ontology: recalculated in ${elapsed}ms (${ontology.length} labels, ${propertiesResult.records.length} properties, ${relationshipsResult.records.length} relationships)`
      )
    )

    // Emit COMPUTE_OPERATION only when a full recalculation was performed
    if (workspaceId) {
      this.kuEventsService.emit(workspaceId, projectId, KuOperation.COMPUTE_OPERATION, {
        type: 'ontology'
      })
    }

    // ── 3. Persist full ontology to SQL projects table for subsequent cache hits ──
    const nowIso = new Date().toISOString()
    try {
      await this.projectRepository.update(projectId, {
        ontologyCache: JSON.stringify(ontology),
        ontologyCachedAt: nowIso
      })
    } catch (err) {
      // Cache write is best-effort — log but never fail the caller
      console.warn('[AiService] failed to persist ontology cache:', err)
    }

    // ── 4. Return (apply in-memory label filter if requested) ─────────────────
    const result = labels?.length ? ontology.filter((item) => labels.includes(item.label)) : ontology
    isDevMode(() =>
      Logger.debug(
        `[AiService] ontology: total ${Date.now() - tTotal}ms (recalc ${elapsed}ms, ${result.length}/${ontology.length} labels returned)`
      )
    )
    return result
  }

  buildMdSchema(ontology: OntologyItem[]): string {
    const esc = (s: string) => String(s).replace(/\|/g, '\\|').replace(/`/g, '\\`')
    const MAX_DISPLAY_VALUES = 5
    const MAX_VALUE_LEN = 48 // truncate individual string values longer than this

    const truncate = (s: string): string =>
      s.length > MAX_VALUE_LEN ? s.slice(0, MAX_VALUE_LEN) + '\u2026' : s

    const fmtValues = (prop: OntologyProperty): string => {
      const t = (prop.type || '').toLowerCase()

      if (t === 'number' || t === 'datetime') {
        if (prop.min !== undefined && prop.max !== undefined) {
          return `\`${prop.min}\`..\`${prop.max}\``
        }
        return '—'
      }

      if (Array.isArray(prop.values) && prop.values.length > 0) {
        const shown = prop.values.slice(0, MAX_DISPLAY_VALUES)
        const tail =
          prop.values.length > MAX_DISPLAY_VALUES ? ` (+${prop.values.length - MAX_DISPLAY_VALUES} more)` : ''
        return shown.map((v) => `\`${esc(truncate(String(v)))}\``).join(', ') + tail
      }

      return '—'
    }

    let md = `# Graph Ontology\n\n`

    // Labels summary table
    md += `## Labels\n\n`
    md += `| Label | Count |\n|-------|------:|\n`
    for (const item of ontology) {
      md += `| \`${esc(item.label)}\` | ${item.count} |\n`
    }
    md += `\n`

    // Per-label detail
    for (const item of ontology) {
      const recordWord = item.count === 1 ? 'record' : 'records'
      md += `---\n\n## \`${esc(item.label)}\` (${item.count} ${recordWord})\n\n`

      if (item.properties.length > 0) {
        md += `### Properties\n\n`
        md += `| Property | Type | Values / Range |\n|----------|------|----------------|\n`
        for (const p of item.properties) {
          md += `| \`${esc(p.name)}\` | ${esc(p.type)} | ${fmtValues(p)} |\n`
        }
        md += `\n`
      }

      if (item.relationships.length > 0) {
        md += `### Relationships\n\n`
        md += `| Type | Direction | Other Label |\n|------|-----------|-------------|\n`
        for (const r of item.relationships) {
          md += `| \`${esc(r.type)}\` | ${r.direction} | \`${esc(r.label)}\` |\n`
        }
        md += `\n`
      }
    }

    return md.trim()
  }

  // ──────────────────────────────────────────────────────────────────────────
  // Embedding Index Management
  // ──────────────────────────────────────────────────────────────────────────

  async findIndexes(projectId: string): Promise<EmbeddingIndexRow[]> {
    return this.embeddingIndexRepository.findByProjectId(projectId)
  }

  /**
   * Computes the propKey stored on VALUE relationships as rel.__propKey.
   * Format: "Label:propertyName" — uniquely identifies both which property is indexed
   * and which label's records were embedded for strict label scoping.
   */
  private computePropKey(propertyName: string, label: string): string {
    return `${label}:${propertyName}`
  }

  private async getEmbeddingBackfillSize(
    projectId: string,
    propertyName: string,
    label: string,
    vectorPropertyName: string,
    transaction: Transaction
  ): Promise<number> {
    const propKey = this.computePropKey(propertyName, label)
    const labelSuffix = label ? `:${label}` : ''
    const result = await transaction.run(
      this.aiQueryService.getEmbeddingIndexStatsQuery(labelSuffix, vectorPropertyName),
      {
        projectId,
        propertyName,
        propKey
      }
    )

    if (result.records.length === 0) {
      return 0
    }

    const rec = result.records[0]
    const toNum = (v: any) => (typeof v?.toNumber === 'function' ? v.toNumber() : Number(v ?? 0))
    const totalRecords = toNum(rec.get('totalRecords'))
    const indexedRecords = toNum(rec.get('indexedRecords'))
    return Math.max(0, totalRecords - indexedRecords)
  }

  private async assertEmbeddingIndexCreationAllowed(
    projectId: string,
    workspaceId: string | undefined,
    propertyName: string,
    label: string,
    vectorPropertyName: string,
    transaction: Transaction
  ): Promise<void> {
    if (!workspaceId) {
      return
    }

    const project = await this.projectRepository.findById(projectId)
    if (project?.customDb) {
      return
    }

    const pendingEmbeddings = await this.getEmbeddingBackfillSize(
      projectId,
      propertyName,
      label,
      vectorPropertyName,
      transaction
    )
    if (pendingEmbeddings === 0) {
      return
    }

    const estimatedKu = estimateEmbeddingBatchKu(pendingEmbeddings)
    await this.billingPolicyService.assertEmbeddingIndexCreationAllowed(workspaceId, estimatedKu)
  }

  async createIndex(
    projectId: string,
    workspaceId: string,
    dto: CreateEmbeddingIndexDto,
    transaction: Transaction
  ): Promise<EmbeddingIndexRow> {
    // 1. Validate the property exists and has a compatible type
    const typeResult = await transaction.run(this.aiQueryService.getPropertyTypeQuery(), {
      projectId,
      propertyName: dto.propertyName
    })

    const sourceType: EmbeddingIndexSourceType =
      dto.external === true ? 'external' : ((dto.sourceType ?? 'managed') as EmbeddingIndexSourceType)
    const similarityFunction: EmbeddingIndexSimilarityFunction = dto.similarityFunction ?? 'cosine'

    const configuredDimensions = Number.parseInt(
      this.configService.get<string>('RUSHDB_EMBEDDING_DIMENSIONS') ?? '0',
      10
    )
    const dimensions = dto.dimensions ?? configuredDimensions
    if (!isValidEmbeddingDimensions(dimensions)) {
      throw new UnprocessableEntityException('dimensions must be an integer between 1 and 4096')
    }

    const vectorPropertyName = buildVectorPropertyName({ sourceType, similarityFunction, dimensions })
    const vectorIndexName = buildVectorIndexName({ sourceType, similarityFunction, dimensions })

    if (typeResult.records.length === 0) {
      throw new NotFoundException(`Property "${dto.propertyName}" does not exist in this project`)
    }

    const propType = typeResult.records[0].get('propType') as string
    if (propType !== 'string') {
      throw new UnprocessableEntityException(
        `Property "${dto.propertyName}" has type "${propType}" — only string (and List<String>) properties can be indexed`
      )
    }

    const label = dto.label

    // 2. Prevent exact duplicate for the same index signature.
    const existing = await this.embeddingIndexRepository.findByProjectIdPropertyAndLabel(
      projectId,
      dto.propertyName,
      label,
      { sourceType, similarityFunction, dimensions }
    )
    if (existing) {
      throw new ConflictException(
        `An embedding index for label "${label}", property "${dto.propertyName}", sourceType "${sourceType}", similarityFunction "${similarityFunction}", and dimensions ${dimensions} already exists`
      )
    }

    await this.assertEmbeddingIndexCreationAllowed(
      projectId,
      workspaceId,
      dto.propertyName,
      label,
      vectorPropertyName,
      transaction
    )

    const modelKey =
      sourceType === 'managed' ? (this.configService.get<string>('RUSHDB_EMBEDDING_MODEL') ?? '') : ''
    if (sourceType === 'managed' && !modelKey) {
      throw new UnprocessableEntityException(
        'Embedding is not fully configured on this server. Set RUSHDB_EMBEDDING_MODEL, RUSHDB_EMBEDDING_DIMENSIONS (positive integer), and RUSHDB_EMBEDDING_API_KEY.'
      )
    }

    if (sourceType === 'managed') {
      try {
        const probe = await this.embeddingProviderService.embed('rushdb-embedding-healthcheck')
        if (!Array.isArray(probe) || probe.length !== dimensions) {
          throw new UnprocessableEntityException(
            `Embedding provider returned vector length ${probe?.length ?? 0}, expected ${dimensions}.`
          )
        }
      } catch (err) {
        if (err instanceof UnprocessableEntityException) {
          throw err
        }
        throw new UnprocessableEntityException(
          `Embedding provider validation failed for model "${modelKey}". ${err instanceof Error ? err.message : ''}`
        )
      }
    }

    const now = new Date().toISOString()
    const row = await this.embeddingIndexRepository.create({
      id: randomUUID(),
      projectId,
      label,
      propertyName: dto.propertyName,
      modelKey,
      sourceType,
      similarityFunction,
      dimensions,
      vectorPropertyName,
      enabled: true,
      status: sourceType === 'managed' ? 'pending' : 'awaiting_vectors',
      createdAt: now,
      updatedAt: now
    })

    // 4. Create per-slot Neo4j vector index (DDL must run outside explicit transaction; idempotent)
    try {
      const session = this.neogmaService.createSession('embedding-index-ddl')
      try {
        await session.run(
          this.aiQueryService.getCreateVectorIndexQuery({
            indexName: vectorIndexName,
            vectorPropertyName,
            similarityFunction
          }),
          {
            dimensions
          }
        )
      } finally {
        await session.close()
      }
    } catch (err) {
      Logger.warn(`[AiService] vector index DDL failed (will be retried by backfill): ${err}`)
    }

    // 5. Emit KU metering event only for managed sourceType.
    if (sourceType === 'managed') {
      this.kuEventsService.emit(workspaceId, projectId, KuOperation.EMBEDDING_GENERATED, {
        label: dto.label,
        propertyName: dto.propertyName,
        modelKey,
        vectorSource: sourceType,
        dimensions,
        similarityFunction
      })
    }

    return row
  }

  async deleteIndex(id: string, projectId: string): Promise<void> {
    const row = await this.embeddingIndexRepository.findById(id)
    if (!row || row.projectId !== projectId) {
      throw new NotFoundException(`Embedding index "${id}" not found`)
    }

    // Delete the SQL config row first so UI can update immediately and
    // no new backfills are triggered for this policy.
    await this.embeddingIndexRepository.delete(id)

    // Fire-and-forget heavy Neo4j cleanup to avoid blocking the HTTP response.
    void this.cleanUpDeletedIndexResources({
      id: row.id,
      projectId: row.projectId,
      propertyName: row.propertyName,
      label: row.label,
      vectorPropertyName: row.vectorPropertyName
    }).catch((err) => {
      Logger.warn(`[AiService] async index cleanup failed for ${row.id}: ${err}`)
    })
  }

  private async cleanUpDeletedIndexResources(row: {
    id: string
    projectId: string
    propertyName: string
    label: string
    vectorPropertyName: string
  }): Promise<void> {
    const propKey = this.computePropKey(row.propertyName, row.label)
    const labelSuffix = row.label ? `:${row.label}` : ''
    const vectorIndexName = `rushdb_emb_rel_${row.vectorPropertyName.replace(/^_emb_/, '')}`

    // Strip embeddings from VALUE relationships for this (label + propertyName + propKey).
    //    The AND rel.__propKey = $propKey guard ensures we only remove data for this exact index,
    //    not for another label's index that shares the same property node.
    try {
      const session = this.neogmaService.createSession('embedding-strip')
      try {
        await session.run(this.aiQueryService.getStripEmbeddingsQuery(labelSuffix, row.vectorPropertyName), {
          projectId: row.projectId,
          propertyName: row.propertyName,
          propKey
        })
      } finally {
        await session.close()
      }
    } catch (err) {
      Logger.warn(`[AiService] embedding strip failed: ${err}`)
    }

    // Drop the slot-specific Neo4j vector index only when no SQL policies reference this vectorPropertyName.
    try {
      const remainingPolicies = await this.embeddingIndexRepository.countByVectorPropertyName(
        row.vectorPropertyName
      )
      if (remainingPolicies === 0) {
        const checkSession = this.neogmaService.createSession('embedding-index-ddl')
        try {
          await checkSession.run(this.aiQueryService.getDropVectorIndexQuery(vectorIndexName))
          Logger.log(`[AiService] vector index dropped for property ${row.vectorPropertyName}`)
        } finally {
          await checkSession.close()
        }
      }
    } catch (err) {
      Logger.warn(`[AiService] vector index cleanup failed for property ${row.vectorPropertyName}: ${err}`)
    }
  }

  async getIndexStats(
    projectId: string,
    indexId: string,
    transaction: Transaction
  ): Promise<{ totalRecords: number; indexedRecords: number }> {
    const row = await this.embeddingIndexRepository.findById(indexId)
    if (!row || row.projectId !== projectId) {
      return { totalRecords: 0, indexedRecords: 0 }
    }

    const propKey = this.computePropKey(row.propertyName, row.label)
    const labelSuffix = row.label ? `:${row.label}` : ''

    const result = await transaction.run(
      this.aiQueryService.getEmbeddingIndexStatsQuery(labelSuffix, row.vectorPropertyName),
      {
        projectId,
        propertyName: row.propertyName,
        propKey
      }
    )

    if (result.records.length === 0) {
      return { totalRecords: 0, indexedRecords: 0 }
    }

    const rec = result.records[0]
    const toNum = (v: any) => (typeof v?.toNumber === 'function' ? v.toNumber() : Number(v))

    return {
      totalRecords: toNum(rec.get('totalRecords')),
      indexedRecords: toNum(rec.get('indexedRecords'))
    }
  }

  async upsertIndexVectors(
    projectId: string,
    workspaceId: string | undefined,
    indexId: string,
    dto: UpsertIndexVectorsDto,
    transaction: Transaction
  ): Promise<{ updated: number; requested: number }> {
    const row = await this.embeddingIndexRepository.findById(indexId)
    if (!row || row.projectId !== projectId) {
      throw new NotFoundException(`Embedding index "${indexId}" not found`)
    }

    if (row.sourceType !== 'external') {
      throw new UnprocessableEntityException('Vector upsert is supported only for external indexes')
    }

    const invalid = dto.items.find(
      (item) =>
        !Array.isArray(item.vector) ||
        item.vector.length !== row.dimensions ||
        item.vector.some((value) => !Number.isFinite(value))
    )
    if (invalid) {
      throw new UnprocessableEntityException(
        `Every vector must contain exactly ${row.dimensions} finite numeric values`
      )
    }

    const propKey = this.computePropKey(row.propertyName, row.label)
    const labelSuffix = row.label ? `:${row.label}` : ''
    const updates = dto.items.map((item) => ({ recordId: item.recordId, emb: item.vector }))

    const result = await transaction.run(
      this.aiQueryService.getWriteEmbeddingsByRecordIdQuery(labelSuffix, row.vectorPropertyName),
      {
        projectId,
        propertyName: row.propertyName,
        propKey,
        updates
      }
    )

    const rec = result.records[0]
    const toNum = (v: any) => (typeof v?.toNumber === 'function' ? v.toNumber() : Number(v ?? 0))
    const requested = toNum(rec?.get('requestedCount'))
    const updated = toNum(rec?.get('updatedCount'))

    if (updated !== requested) {
      throw new UnprocessableEntityException(
        `Some record ids were not found for label "${row.label}" and property "${row.propertyName}"`
      )
    }

    const stats = await this.getIndexStats(projectId, indexId, transaction)
    await this.embeddingIndexRepository.updateStatus(
      row.id,
      stats.totalRecords > 0 && stats.totalRecords === stats.indexedRecords ? 'ready' : 'awaiting_vectors'
    )

    if (workspaceId) {
      this.kuEventsService.emit(workspaceId, projectId, KuOperation.EMBEDDING_GENERATED, {
        count: updated,
        source: 'external_upsert',
        indexId: row.id,
        label: row.label,
        propertyName: row.propertyName,
        vectorSource: row.sourceType,
        dimensions: row.dimensions,
        similarityFunction: row.similarityFunction
      })
    }

    return { updated, requested }
  }

  /**
   * Performs exact semantic search using an embedding index scoped to the first entry in dto.labels.
   * Candidates are first filtered via Cypher MATCH/WHERE and then ranked by cosine similarity.
   */
  async semanticSearch(
    projectId: string,
    dto: SemanticSearchDto,
    transaction: import('neo4j-driver').Transaction,
    workspaceId?: string
  ): Promise<Array<Record<string, any>>> {
    // Resolve the index by (projectId, propertyName, labels[0])
    const label = dto.labels[0]
    const sourceType: EmbeddingIndexSourceType = dto.sourceType ?? 'managed'
    const similarityFunction: EmbeddingIndexSimilarityFunction = dto.similarityFunction ?? 'cosine'
    const dimensions =
      dto.dimensions ??
      (Array.isArray(dto.queryVector) ? dto.queryVector.length : undefined) ??
      Number.parseInt(this.configService.get<string>('RUSHDB_EMBEDDING_DIMENSIONS') ?? '0', 10)

    const index = await this.embeddingIndexRepository.findByProjectIdPropertyAndLabel(
      projectId,
      dto.propertyName,
      label,
      { sourceType, similarityFunction, dimensions }
    )
    if (!index) {
      throw new NotFoundException(
        `No embedding index found for label "${label}" and property "${dto.propertyName}" matching the requested vector signature`
      )
    }
    if (index.status !== 'ready') {
      throw new UnprocessableEntityException(
        `Embedding index for "${label}:${dto.propertyName}" is not ready yet (status: ${index.status})`
      )
    }

    if (dto.query && dto.queryVector) {
      throw new UnprocessableEntityException('Provide either query or queryVector, but not both')
    }

    const propKey = this.computePropKey(dto.propertyName, label)
    const labelSuffix = label ? `:${label}` : ''
    let queryVector: number[]
    let tokensUsed: number | undefined

    if (Array.isArray(dto.queryVector) && dto.queryVector.length > 0) {
      if (dto.queryVector.length !== index.dimensions) {
        throw new UnprocessableEntityException(
          `Query vector length ${dto.queryVector.length} does not match index dimensions ${index.dimensions}`
        )
      }
      queryVector = dto.queryVector
    } else {
      if (!dto.query) {
        throw new UnprocessableEntityException('query is required when queryVector is not provided')
      }
      if (index.sourceType === 'external') {
        throw new UnprocessableEntityException(
          `Embedding index for "${label}:${dto.propertyName}" requires queryVector because it is an external index`
        )
      }

      const embeddingResult = await this.embeddingProviderService.embedWithUsage(dto.query)
      queryVector = embeddingResult.embedding
      tokensUsed = embeddingResult.tokensUsed
    }

    if (workspaceId && dto.query) {
      const tokenCount = tokensUsed ?? estimateTokens(dto.query)
      this.kuEventsService.emit(workspaceId, projectId, KuOperation.EMBEDDING_GENERATED, {
        count: 1,
        tokenCount,
        estimatedTokens: tokenCount,
        source: 'semantic_query',
        label,
        propertyName: dto.propertyName,
        vectorSource: index.sourceType,
        dimensions: index.dimensions,
        similarityFunction: index.similarityFunction
      })
    }

    const toNonNegativeInt = (value: unknown, fallback: number) => {
      const parsed = Number.parseInt(String(value ?? fallback), 10)
      if (!Number.isInteger(parsed) || parsed < 0) {
        return fallback
      }
      return parsed
    }

    const skip = toNonNegativeInt(dto.skip, 0)
    const limit = toNonNegativeInt(dto.limit, 20)

    const hasWhere = dto.where != null && Object.keys(dto.where).length > 0
    const hasMultiLabels = dto.labels.length > 1

    let query: string
    let params: Record<string, unknown>

    // Always prefilter candidates first for correctness in a multi-tenant shared index.
    {
      // Narrow candidates first, then exact cosine scoring.
      // Sort all queryParts (record < record1 < ...) so that:
      //   sortedParts[0]  = root record WHERE condition
      //   sortedParts[1+] = OPTIONAL MATCH clauses for related-record traversals
      const parsedWhere = hasWhere ? parseWhereClause(dto.where) : null
      const sortedParts =
        parsedWhere ?
          Object.keys(parsedWhere.queryParts)
            .sort((a, b) => a.localeCompare(b))
            .map((k) => parsedWhere.queryParts[k])
        : []
      const rootWhereClause = sortedParts[0] ?? ''
      const extraMatchClauses = sortedParts.slice(1)
      // For multi-label, build a label filter clause
      const multiLabelClause =
        hasMultiLabels ?
          `any(l IN labels(record) WHERE l IN [${dto.labels.map((l) => `'${l}'`).join(', ')}])`
        : ''
      const combinedWhere = [rootWhereClause, multiLabelClause].filter(Boolean).join(' AND ')
      query = this.aiQueryService.getSemanticSearchPrefilterQuery({
        combinedWhere,
        labelSuffix,
        similarityFunction: index.similarityFunction as EmbeddingIndexSimilarityFunction,
        vectorPropertyName: index.vectorPropertyName,
        extraMatchClauses,
        nodeAliases: parsedWhere?.nodeAliases ?? ['record'],
        requiredAliasCheck: parsedWhere?.where ?? ''
      })
      params = {
        queryVector,
        propertyName: dto.propertyName,
        propKey,
        projectId,
        skip: neo4jInt(skip),
        limit: neo4jInt(limit)
      }
    }

    const result = await transaction.run(query, params)

    const toNum = (v: any) => (typeof v?.toNumber === 'function' ? v.toNumber() : Number(v))

    return result.records.map((r) => ({
      ...(r.get('record')?.properties ?? r.get('record')),
      __score: toNum(r.get('score'))
    }))
  }

  /**
   * Resolves matching external embedding indexes for each inline vector entry and writes
   * the vectors to the relationship properties.
   *
   * Logic per entry:
   *  - 0 matches  → NotFoundException
   *  - 1 match    → write unconditionally
   *  - 2+ matches → require `similarityFunction` to disambiguate, otherwise UnprocessableEntityException
   */
  async resolveAndWriteInlineVectors(
    projectId: string,
    label: string,
    recordId: string,
    vectors: InlineVectorEntryDto[],
    transaction: Transaction
  ): Promise<void> {
    for (const entry of vectors) {
      const dimensions = entry.vector.length
      const candidates = await this.embeddingIndexRepository.findMatchingExternalIndexes(
        projectId,
        label,
        entry.propertyName,
        dimensions,
        entry.similarityFunction
      )

      if (candidates.length === 0) {
        throw new NotFoundException(
          `No external embedding index found for label "${label}", property "${entry.propertyName}", dimensions ${dimensions}${entry.similarityFunction ? `, similarityFunction "${entry.similarityFunction}"` : ''}`
        )
      }
      if (candidates.length > 1) {
        throw new UnprocessableEntityException(
          `Multiple external embedding indexes match label "${label}", property "${entry.propertyName}", dimensions ${dimensions}. ` +
            `Specify "similarityFunction" in the vector entry to disambiguate.`
        )
      }

      const row = candidates[0]
      if (!entry.vector.every((v) => Number.isFinite(v))) {
        throw new UnprocessableEntityException(
          `Vector for property "${entry.propertyName}" contains non-finite values`
        )
      }

      const propKey = this.computePropKey(row.propertyName, row.label)
      const labelSuffix = row.label ? `:${row.label}` : ''

      await transaction.run(
        this.aiQueryService.getWriteEmbeddingsByRecordIdQuery(labelSuffix, row.vectorPropertyName),
        {
          projectId,
          propertyName: row.propertyName,
          propKey,
          updates: [{ recordId, emb: entry.vector }]
        }
      )

      const stats = await this.getIndexStats(projectId, row.id, transaction)
      await this.embeddingIndexRepository.updateStatus(
        row.id,
        stats.totalRecords > 0 && stats.totalRecords === stats.indexedRecords ? 'ready' : 'awaiting_vectors'
      )
    }
  }
}
