import {
  ConflictException,
  Injectable,
  Logger,
  NotFoundException,
  UnprocessableEntityException
} from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'
import { randomUUID } from 'crypto'

import { AiQueryService } from '@/core/ai/ai-query.service'
import { EmbeddingIndexRepository } from '@/core/ai/embedding-index.repository'
import { EmbeddingProviderService } from '@/core/ai/embedding-provider.service'
import { NeogmaService } from '@/database/neogma/neogma.service'
import { parseWhereClause } from '@/core/search/parser/buildQuery'
import type { OntologyItem, OntologyProperty, OntologyRelationship } from '@/core/ai/ai.types'
import type { CreateEmbeddingIndexDto } from '@/core/ai/dto/create-embedding-index.dto'
import type { SemanticSearchDto } from '@/core/ai/dto/semantic-search.dto'
import type { EmbeddingIndexRow } from '@/database/sql/schema/types'
import { isDevMode } from '@/common/utils/isDevMode'
import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'

/** How long (ms) a cached ontology is considered fresh before a recalculation is triggered. */
const ONTOLOGY_CACHE_TTL_MS = 60 * 60 * 1000 // 1 hour

@Injectable()
export class AiService {
  constructor(
    private readonly aiQueryService: AiQueryService,
    private readonly configService: ConfigService,
    private readonly kuEventsService: KuEventsService,
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
      if (!labelRelsMap.has(label)) labelRelsMap.set(label, [])
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

      if (fromLabel === toLabel) continue // skip self-loops

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
        if (a.direction !== b.direction) return a.direction < b.direction ? -1 : 1
        if (a.label !== b.label) return a.label.localeCompare(b.label)
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
   * and which label's records were embedded, enabling label-isolated ANN post-filtering.
   */
  private computePropKey(propertyName: string, label: string): string {
    return `${label}:${propertyName}`
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

    if (typeResult.records.length === 0) {
      throw new NotFoundException(`Property "${dto.propertyName}" does not exist in this project`)
    }

    const propType = typeResult.records[0].get('propType') as string
    if (propType !== 'string') {
      throw new UnprocessableEntityException(
        `Property "${dto.propertyName}" has type "${propType}" — only string (and List<String>) properties can be indexed`
      )
    }

    // 2. Prevent exact duplicate: same (projectId, label, propertyName)
    const existing = await this.embeddingIndexRepository.findByProjectIdPropertyAndLabel(
      projectId,
      dto.propertyName,
      dto.label
    )
    if (existing) {
      throw new ConflictException(
        `An embedding index for label "${dto.label}" and property "${dto.propertyName}" already exists`
      )
    }

    // 3. Persist the index policy in SQL
    const modelKey = this.configService.get<string>('RUSHDB_EMBEDDING_MODEL') ?? ''
    const dimensions = Number(this.configService.get('RUSHDB_EMBEDDING_DIMENSIONS') ?? 0)
    const apiKey = this.configService.get<string>('RUSHDB_EMBEDDING_API_KEY') ?? ''
    if (!modelKey || !dimensions || !apiKey) {
      throw new UnprocessableEntityException(
        'Embedding is not fully configured on this server. Set RUSHDB_EMBEDDING_MODEL, RUSHDB_EMBEDDING_DIMENSIONS, and RUSHDB_EMBEDDING_API_KEY.'
      )
    }

    const now = new Date().toISOString()
    const row = await this.embeddingIndexRepository.create({
      id: randomUUID(),
      projectId,
      label: dto.label,
      propertyName: dto.propertyName,
      modelKey: modelKey,
      dimensions: dimensions,
      enabled: true,
      status: 'pending',
      createdAt: now,
      updatedAt: now
    })

    // 4. Create Neo4j global vector index (DDL must run outside explicit transaction; idempotent)
    try {
      const session = this.neogmaService.createSession('embedding-index-ddl')
      try {
        await session.run(this.aiQueryService.getCreateGlobalVectorIndexQuery(), { dimensions })
      } finally {
        await session.close()
      }
    } catch (err) {
      Logger.warn(`[AiService] vector index DDL failed (will be retried by backfill): ${err}`)
    }

    // 5. Emit KU metering event
    this.kuEventsService.emit(workspaceId, projectId, KuOperation.EMBEDDING_GENERATED, {
      label: dto.label,
      propertyName: dto.propertyName,
      modelKey: modelKey
    })

    return row
  }

  async deleteIndex(id: string, projectId: string): Promise<void> {
    const row = await this.embeddingIndexRepository.findById(id)
    if (!row || row.projectId !== projectId) {
      throw new NotFoundException(`Embedding index "${id}" not found`)
    }

    const propKey = this.computePropKey(row.propertyName, row.label)
    const labelSuffix = `:${row.label}`

    // 1. Delete the SQL config row first so no new backfills are triggered for this policy.
    await this.embeddingIndexRepository.delete(id)

    // 2. Strip embeddings from VALUE relationships for this (label + propertyName + propKey).
    //    The AND rel.__propKey = $propKey guard ensures we only remove data for this exact index,
    //    not for another label's index that shares the same property node.
    try {
      const session = this.neogmaService.createSession('embedding-strip')
      try {
        await session.run(this.aiQueryService.getStripEmbeddingsQuery(labelSuffix), {
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

    // 3. Drop the global Neo4j vector index only when no embeddings remain anywhere in the graph.
    //    This is a global check (not scoped to project) because the DDL index is shared across
    //    all tenants. If user A deletes their last index but user B still has embeddings,
    //    the index must be preserved.
    try {
      const checkSession = this.neogmaService.createSession('embedding-index-ddl')
      try {
        const result = await checkSession.run(this.aiQueryService.getAnyEmbeddingExistsQuery())
        if (result.records.length === 0) {
          // No embeddings left anywhere — safe to drop the DDL index
          await checkSession.run(this.aiQueryService.getDropGlobalVectorIndexQuery())
          Logger.log('[AiService] global vector index dropped (no embeddings remain)')
        }
      } finally {
        await checkSession.close()
      }
    } catch (err) {
      Logger.warn(`[AiService] global vector index cleanup failed: ${err}`)
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
    const labelSuffix = `:${row.label}`

    const result = await transaction.run(this.aiQueryService.getEmbeddingIndexStatsQuery(labelSuffix), {
      projectId,
      propertyName: row.propertyName,
      propKey
    })

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

  /**
   * Performs semantic search using an embedding index scoped to the first entry in dto.labels.
   * - ANN mode (fast): no `where` filter and exactly one label.
   * - ENN prefilter mode (exact): `where` filter present, or multiple labels.
   */
  async semanticSearch(
    projectId: string,
    dto: SemanticSearchDto,
    transaction: import('neo4j-driver').Transaction
  ): Promise<Array<Record<string, any>>> {
    // Resolve the index by (projectId, propertyName, labels[0])
    const label = dto.labels[0]
    const index = await this.embeddingIndexRepository.findByProjectIdPropertyAndLabel(
      projectId,
      dto.propertyName,
      label
    )
    if (!index) {
      throw new NotFoundException(
        `No embedding index found for label "${label}" and property "${dto.propertyName}"`
      )
    }
    if (index.status !== 'ready') {
      throw new UnprocessableEntityException(
        `Embedding index for "${label}:${dto.propertyName}" is not ready yet (status: ${index.status})`
      )
    }

    const propKey = this.computePropKey(dto.propertyName, label)
    const labelSuffix = `:${label}`
    const queryVector = await this.embeddingProviderService.embed(dto.query)
    const topK = dto.topK ?? 20
    const skip = dto.skip ?? 0
    const limit = dto.limit ?? 20

    const hasWhere = dto.where != null && Object.keys(dto.where).length > 0
    const hasMultiLabels = dto.labels.length > 1

    let query: string
    let params: Record<string, unknown>

    if (hasWhere || hasMultiLabels) {
      // ENN prefilter path: narrow candidates first, then exact cosine scoring
      const { where: whereClause } = hasWhere ? parseWhereClause(dto.where) : { where: '' }
      // For multi-label, build a label filter clause
      const multiLabelClause =
        hasMultiLabels ?
          `any(l IN labels(record) WHERE l IN [${dto.labels.map((l) => `'${l}'`).join(', ')}])`
        : ''
      const combinedWhere = [whereClause, multiLabelClause].filter(Boolean).join(' AND ')
      query = this.aiQueryService.getSemanticSearchPrefilterQuery(combinedWhere, labelSuffix)
      params = { queryVector, propertyName: dto.propertyName, propKey, projectId, skip, limit }
    } else {
      // ANN path: fast approximate nearest neighbour via global vector index
      query = this.aiQueryService.getSemanticSearchAnnQuery(labelSuffix)
      params = { queryVector, propKey, projectId, topK, skip, limit }
    }

    const result = await transaction.run(query, params)

    const toNum = (v: any) => (typeof v?.toNumber === 'function' ? v.toNumber() : Number(v))

    return result.records.map((r) => ({
      ...(r.get('record')?.properties ?? r.get('record')),
      __score: toNum(r.get('score'))
    }))
  }
}
