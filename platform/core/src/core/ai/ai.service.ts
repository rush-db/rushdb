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
import { int as neo4jInt, Session, Transaction } from 'neo4j-driver'

import { isDevMode } from '@/common/utils/isDevMode'
import { AiQueryService } from '@/core/ai/ai-query.service'
import { EmbeddingIndexDdlService } from '@/core/ai/embedding-index-ddl.service'
import { EmbeddingIndexRepository } from '@/core/ai/embedding-index.repository'
import {
  buildVectorIndexName,
  buildVectorPropertyName,
  isValidEmbeddingDimensions,
  type EmbeddingIndexSimilarityFunction,
  type EmbeddingIndexSourceType
} from '@/core/ai/embedding-index.utils'
import { EmbeddingProviderService } from '@/core/ai/embedding-provider.service'
import { estimateEmbeddingBatchKu, estimateTokens } from '@/core/ai/embedding.utils'
import { BILLING_POLICY_PORT, BillingPolicyPort } from '@/core/billing-policy/billing-policy.port'
import { ISO_8601_REGEX, RUSHDB_KEY_LABEL_ALIAS, RUSHDB_LABEL_RECORD } from '@/core/common/constants'
import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { parseWhereClause } from '@/core/search/parser/buildQuery'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'
import { dbContextStorage } from '@/database/db-context'
import { NeogmaService } from '@/database/neogma/neogma.service'
import { DEFAULT_TRANSACTION_TIMEOUT_MS } from '@/database/transaction.constants'

import { randomUUID } from 'crypto'

import type { SchemaItem, SchemaProperty, SchemaRelationship, SchemaVectorIndex } from '@/core/ai/ai.types'
import type { CreateEmbeddingIndexDto } from '@/core/ai/dto/create-embedding-index.dto'
import type { InlineVectorEntryDto } from '@/core/ai/dto/inline-vector-entry.dto'
import type { SemanticSearchDto } from '@/core/ai/dto/semantic-search.dto'
import type { UpsertIndexVectorsDto } from '@/core/ai/dto/upsert-index-vectors.dto'
import type { EmbeddingIndexRow } from '@/database/sql/schema/types'

/** How long (ms) a cached schema is considered fresh before a recalculation is triggered. */
const SCHEMA_CACHE_TTL_MS = 60 * 60 * 1000 // 1 hour

/**
 * Max VALUE relationships scanned by the pre-creation billing estimate. The request
 * transaction has a 30s timeout; an unbounded count over a large label blows it.
 * Anything beyond the cap is enforced per batch by the backfill scheduler.
 */
const EMBEDDING_BILLING_SCAN_CAP = 10_000

/** How long (ms) a successful embedding-provider healthcheck is trusted before re-probing. */
const EMBEDDING_PROBE_TTL_MS = 10 * 60 * 1000

/** Records scanned per (label, property) when sampling string values for the schema. */
const SCHEMA_SAMPLE_SCAN_RECORDS = 100

/** Max distinct sample values kept per string property in the schema. */
const SCHEMA_SAMPLE_VALUES = 10

/** Labels recalculated concurrently during a schema rebuild (each statement on its own session). */
const SCHEMA_RECALC_LABEL_CONCURRENCY = 4

/** Sample (label, propId) pairs sent per bounded-sampling statement. */
const SCHEMA_SAMPLE_PAIRS_CHUNK = 500

/** Produces a fresh short-lived session for one schema statement. */
type SchemaSessionFactory = () => Session

const mapWithConcurrency = async <T>(
  items: T[],
  limit: number,
  fn: (item: T) => Promise<void>
): Promise<void> => {
  let index = 0
  const workers = Array.from({ length: Math.max(1, Math.min(limit, items.length)) }, async () => {
    while (index < items.length) {
      await fn(items[index++])
    }
  })
  await Promise.all(workers)
}

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
    private readonly embeddingIndexDdlService: EmbeddingIndexDdlService,
    private readonly neogmaService: NeogmaService
  ) {}

  /** Timestamps of last successful embedding-provider probe, keyed by `${modelKey}:${dimensions}`. */
  private readonly embeddingProbeCache = new Map<string, number>()

  /** In-flight schema recalculations, keyed by projectId (single-flight guard). */
  private readonly schemaRecalcInFlight = new Map<string, Promise<SchemaItem[]>>()

  /** Coalesced follow-up runs queued by force callers behind an in-flight recalculation. */
  private readonly schemaRecalcQueued = new Map<string, Promise<SchemaItem[]>>()

  private inferSchemaValueType(values: unknown[]): string {
    const nonNullValues = values.filter((value) => value !== null && value !== undefined)
    if (nonNullValues.length === 0) {
      // null is no longer a stored type; an all-null/empty sample defaults to string.
      return 'string'
    }
    if (nonNullValues.every((value) => typeof value === 'number')) {
      return 'number'
    }
    if (nonNullValues.every((value) => typeof value === 'boolean')) {
      return 'boolean'
    }
    if (
      nonNullValues.every((value) => typeof value === 'string') &&
      nonNullValues.every((value) => ISO_8601_REGEX.test(String(value)))
    ) {
      return 'datetime'
    }
    return 'string'
  }

  /**
   * `values` is a bounded sample (up to 10 distinct values) used for type inference and
   * display; min/max come from exact streaming aggregations over ALL values computed in
   * Cypher — numeric for number props, lexicographic for datetimes (ISO strings sort
   * chronologically, matching the previous String-sort behavior).
   */
  private buildRelationshipPropertySummary({
    name,
    values,
    relationshipsCount,
    minNumeric,
    maxNumeric,
    minString,
    maxString
  }: {
    name: string
    values: unknown[]
    relationshipsCount: number
    minNumeric: number | null
    maxNumeric: number | null
    minString: string | null
    maxString: string | null
  }): NonNullable<SchemaRelationship['properties']>[number] {
    const type = this.inferSchemaValueType(values)
    const prop: NonNullable<SchemaRelationship['properties']>[number] = {
      name,
      type,
      relationshipsCount
    }
    const normalizedValues = values.filter((value) => value !== undefined && value !== null)

    if (type === 'number') {
      if (minNumeric != null && maxNumeric != null) {
        prop.min = minNumeric
        prop.max = maxNumeric
      }
    } else if (type === 'datetime') {
      if (minString != null && maxString != null) {
        prop.min = minString
        prop.max = maxString
      }
    } else if (type === 'string' || type === 'boolean') {
      prop.values = normalizedValues.slice(0, 10).map((value) => value as string | number | boolean)
    }

    return prop
  }

  /**
   * Returns the project's graph schema. Serving strategy:
   *  - fresh cache → served directly
   *  - stale cache → served immediately; a single-flight background recalculation
   *    refreshes it (allowStale additionally treats any cache as fresh and never
   *    triggers a refresh)
   *  - no cache / force → awaits a full recalculation, joining one already in flight
   *
   * Recalculation never runs on the request-scoped transaction: it fans out small
   * per-label statements on dedicated short-lived READ sessions (see
   * recalculateSchema), so no single unit of work approaches the server-side
   * transaction time budget regardless of project size.
   */
  async getSchema({
    projectId,
    workspaceId,
    labels,
    force,
    allowStale
  }: {
    projectId: string
    /** When provided, a COMPUTE_OPERATION KU event is emitted on recalculation. */
    workspaceId?: string
    labels?: string[]
    /** When true, bypasses the cache and forces a full recalculation. */
    force?: boolean
    /**
     * When true, any persisted cache is served regardless of age and a recalculation is
     * never triggered (unless no cache exists at all). For hot read paths — e.g. the
     * relationship-patterns list that the dashboard polls every few seconds.
     */
    allowStale?: boolean
  }): Promise<SchemaItem[]> {
    const tTotal = Date.now()

    // Capture the connection source now: a background refresh outlives the request,
    // and the AsyncLocalStorage db context is only guaranteed while it is alive.
    const sessionFactory = this.resolveSchemaSessionFactory()

    // ── 1. Check persisted cache in SQL projects table ────────────────────────
    const projectRow = await this.projectRepository.findById(projectId)
    const cachedAt: string | null = projectRow?.schemaCachedAt ?? null
    const cacheJson: string | null = projectRow?.schemaCache ?? null

    const filterByLabels = (schema: SchemaItem[]) =>
      labels?.length ? schema.filter((item) => labels.includes(item.label)) : schema

    if (!force && cachedAt && cacheJson) {
      const ageMs = Date.now() - new Date(cachedAt).getTime()
      const fullSchema: SchemaItem[] = JSON.parse(cacheJson)
      const result = filterByLabels(fullSchema)
      const fresh = ageMs < SCHEMA_CACHE_TTL_MS

      if (!fresh && !allowStale) {
        // Stale: serve what we have and refresh in the background (single-flight) —
        // callers are never blocked behind a full-graph recomputation.
        this.recalculateSchemaSingleFlight({ projectId, workspaceId, sessionFactory }).catch((error) =>
          Logger.error(`[AiService] background schema refresh failed for project ${projectId}`, error)
        )
      }

      isDevMode(() =>
        Logger.debug(
          `[AiService] schema: cache ${fresh ? 'hit' : 'stale-served'} — age ${Math.round(ageMs / 1000)}s, ${result.length}/${fullSchema.length} labels, total ${Date.now() - tTotal}ms`
        )
      )
      return result
    }

    // ── 2. force or no cache at all — the caller waits for the recalculation.
    const schema = await this.recalculateSchemaSingleFlight({ projectId, workspaceId, sessionFactory, force })
    const result = filterByLabels(schema)
    isDevMode(() =>
      Logger.debug(
        `[AiService] schema: total ${Date.now() - tTotal}ms (${result.length}/${schema.length} labels returned)`
      )
    )
    return result
  }

  /**
   * One recalculation per project at a time; concurrent callers share the same run.
   *
   * force callers get read-your-writes semantics: a run already in flight may have
   * started before the caller's writes committed, so instead of joining it a force
   * caller queues exactly one follow-up run (coalesced across concurrent force
   * callers) that starts after the current run settles.
   */
  private recalculateSchemaSingleFlight({
    projectId,
    workspaceId,
    sessionFactory,
    force
  }: {
    projectId: string
    workspaceId?: string
    sessionFactory: SchemaSessionFactory
    force?: boolean
  }): Promise<SchemaItem[]> {
    const inFlight = this.schemaRecalcInFlight.get(projectId)
    if (!inFlight) {
      const run = this.recalculateSchema({ projectId, workspaceId, sessionFactory }).finally(() =>
        this.schemaRecalcInFlight.delete(projectId)
      )
      this.schemaRecalcInFlight.set(projectId, run)
      return run
    }
    if (!force) {
      return inFlight
    }
    const queued = this.schemaRecalcQueued.get(projectId)
    if (queued) {
      return queued
    }
    const followUp = inFlight
      .catch(() => {
        /* the follow-up run reports its own outcome */
      })
      .then(() => {
        this.schemaRecalcQueued.delete(projectId)
        return this.recalculateSchemaSingleFlight({ projectId, workspaceId, sessionFactory, force: true })
      })
    this.schemaRecalcQueued.set(projectId, followUp)
    return followUp
  }

  /**
   * Schema statements run against the project's own database: the BYO external
   * connection when the request targets one (resolved from the request-scoped
   * AsyncLocalStorage db context), the default connection otherwise.
   */
  private resolveSchemaSessionFactory(): SchemaSessionFactory {
    const externalDriver = dbContextStorage.getStore()?.externalConnection?.driver
    if (externalDriver) {
      return () => externalDriver.session({ defaultAccessMode: 'READ' }) as unknown as Session
    }
    return () => this.neogmaService.createSession('schema-recalc', 'READ')
  }

  /** Runs one statement as its own auto-commit transaction on a fresh session. */
  private async runSchemaQuery(
    sessionFactory: SchemaSessionFactory,
    query: string,
    params: Record<string, any>
  ) {
    const session = sessionFactory()
    try {
      return await session.run(query, params, { timeout: DEFAULT_TRANSACTION_TIMEOUT_MS })
    } finally {
      await session.close()
    }
  }

  /**
   * Full schema recalculation, labels-first: one cheap label inventory, then a
   * per-label fan-out (bounded concurrency) of small single-purpose statements —
   *  - property inventory via pure VALUE-edge counts (no value reads),
   *  - exact streaming min/max + isArray for number/datetime properties only,
   *  - relationship topology as a plain count(*),
   *  - relationship-property summaries that only aggregate relationships carrying
   *    custom properties,
   * plus one bounded sampling pass for string/boolean properties (values + isArray
   * over the sampled window). Persists the result to the SQL cache before returning.
   */
  private async recalculateSchema({
    projectId,
    workspaceId,
    sessionFactory
  }: {
    projectId: string
    workspaceId?: string
    sessionFactory: SchemaSessionFactory
  }): Promise<SchemaItem[]> {
    const t0 = Date.now()
    const params: Record<string, any> = { projectId }

    const [labelsResult, allIndexes] = await Promise.all([
      this.runSchemaQuery(sessionFactory, this.aiQueryService.getLabelsQuery(), params),
      this.embeddingIndexRepository.findByProjectId(projectId)
    ])

    const toNumber = (value: any): number => value?.toNumber?.() ?? Number(value)

    // ---------- Build label → count map (Q1) ----------
    const labelCountMap = new Map<string, number>()
    for (const r of labelsResult.records) {
      labelCountMap.set(r.get('label') as string, toNumber(r.get('recordCount')))
    }

    const labelPropsMap = new Map<string, SchemaProperty[]>()
    const relationshipPropertiesMap = new Map<string, NonNullable<SchemaRelationship['properties']>>()
    const relCountRows: Array<{ fromLabel: string; relType: string; toLabel: string; relCount: number }> = []
    const toNullableNumber = (v: any): number | null =>
      v == null ? null
      : typeof v?.toNumber === 'function' ? v.toNumber()
      : Number(v)

    // ---------- Per-label fan-out (Q2a/Q2b/Q3a/Q3b) ----------
    // Workers mutate the shared maps; that is safe single-threaded, and each statement
    // runs on its own session so labels genuinely execute in parallel on the server.
    await mapWithConcurrency([...labelCountMap.keys()], SCHEMA_RECALC_LABEL_CONCURRENCY, async (label) => {
      // Property inventory: pure VALUE-edge counts, no value reads (Q2a)
      const countsResult = await this.runSchemaQuery(
        sessionFactory,
        this.aiQueryService.getLabelPropertyCountsQuery(label),
        params
      )
      const props: SchemaProperty[] = countsResult.records.map((r) => {
        const prop: SchemaProperty = {
          id: r.get('propId') as string,
          name: r.get('propName') as string,
          type: r.get('propType') as string,
          recordsCount: toNumber(r.get('recordsCount'))
        }
        if (prop.type === 'boolean') {
          prop.values = ['true', 'false']
        }
        return prop
      })
      if (props.length) {
        labelPropsMap.set(label, props)
      }

      // Exact streaming min/max + isArray for number/datetime properties (Q2b)
      const statsProps = props.filter((prop) => prop.type === 'number' || prop.type === 'datetime')
      if (statsProps.length) {
        const statsResult = await this.runSchemaQuery(
          sessionFactory,
          this.aiQueryService.getLabelPropertyStatsQuery(label),
          { projectId, props: statsProps.map(({ id, name, type }) => ({ id, name, type })) }
        )
        const statsById = new Map(statsResult.records.map((r) => [r.get('propId') as string, r]))
        for (const prop of statsProps) {
          const row = statsById.get(prop.id)
          if (!row) {
            continue
          }
          const minValue = row.get('minValue')
          const maxValue = row.get('maxValue')
          if (row.get('isArray') === true) {
            prop.isArray = true
          }
          if (minValue != null) {
            prop.min = typeof minValue === 'object' && minValue?.toNumber ? minValue.toNumber() : minValue
          }
          if (maxValue != null) {
            prop.max = typeof maxValue === 'object' && maxValue?.toNumber ? maxValue.toNumber() : maxValue
          }
        }
      }

      // Relationship topology: exact counts, no keys()/DISTINCT (Q3a)
      const relCountsResult = await this.runSchemaQuery(
        sessionFactory,
        this.aiQueryService.getLabelRelationshipCountsQuery(label),
        params
      )
      for (const r of relCountsResult.records) {
        relCountRows.push({
          fromLabel: label,
          relType: r.get('relType') as string,
          toLabel: r.get('toLabel') as string,
          relCount: toNumber(r.get('relCount'))
        })
      }

      // Relationship-property summaries (Q3b)
      const relPropsResult = await this.runSchemaQuery(
        sessionFactory,
        this.aiQueryService.getLabelRelationshipPropertiesQuery(label),
        params
      )
      for (const r of relPropsResult.records) {
        const relType = r.get('relType') as string
        const toLabel = r.get('toLabel') as string
        const key = `${label}|${relType}|${toLabel}`
        const current = relationshipPropertiesMap.get(key) ?? []
        current.push(
          this.buildRelationshipPropertySummary({
            name: r.get('propName') as string,
            values: (r.get('values') ?? []) as unknown[],
            relationshipsCount: toNumber(r.get('relationshipsCount')),
            minNumeric: toNullableNumber(r.get('minNumeric')),
            maxNumeric: toNullableNumber(r.get('maxNumeric')),
            minString: (r.get('minString') as string | null) ?? null,
            maxString: (r.get('maxString') as string | null) ?? null
          })
        )
        relationshipPropertiesMap.set(key, current)
      }
    })

    // ---------- Bounded samples for string/boolean properties ----------
    // Values for strings, isArray for both — detected over the sampled window
    // (see getPropertySampleValuesQuery), never materializing full columns.
    const samplePairs: Array<{ label: string; propId: string }> = []
    for (const [label, props] of labelPropsMap) {
      for (const prop of props) {
        if (prop.type === 'string' || prop.type === 'boolean') {
          samplePairs.push({ label, propId: prop.id })
        }
      }
    }

    for (let offset = 0; offset < samplePairs.length; offset += SCHEMA_SAMPLE_PAIRS_CHUNK) {
      const chunk = samplePairs.slice(offset, offset + SCHEMA_SAMPLE_PAIRS_CHUNK)
      const samplesResult = await this.runSchemaQuery(
        sessionFactory,
        this.aiQueryService.getPropertySampleValuesQuery(),
        {
          projectId,
          pairs: chunk,
          sampleScanLimit: neo4jInt(SCHEMA_SAMPLE_SCAN_RECORDS),
          sampleValuesLimit: neo4jInt(SCHEMA_SAMPLE_VALUES)
        }
      )
      const rowsByKey = new Map(samplesResult.records.map((r) => [`${r.get('label')}|${r.get('propId')}`, r]))
      for (const { label, propId } of chunk) {
        const row = rowsByKey.get(`${label}|${propId}`)
        if (!row) {
          continue
        }
        const prop = labelPropsMap.get(label)?.find((candidate) => candidate.id === propId)
        if (!prop) {
          continue
        }
        if (row.get('isArray') === true) {
          prop.isArray = true
        }
        if (prop.type === 'string') {
          const samples = (row.get('samples') ?? []) as unknown[]
          if (samples.length) {
            prop.values = samples as SchemaProperty['values']
          }
        }
      }
    }

    // ---------- Build label → relationships map ----------
    // Relationships are stored bidirectionally: fromLabel gets 'out', toLabel gets 'in'.
    const labelRelsMap = new Map<string, SchemaRelationship[]>()

    const addRel = (label: string, rel: SchemaRelationship) => {
      if (!labelRelsMap.has(label)) {
        labelRelsMap.set(label, [])
      }
      const existing = labelRelsMap.get(label)!
      const key = `${rel.direction}|${rel.label}|${rel.type}`
      if (!existing.some((e) => `${e.direction}|${e.label}|${e.type}` === key)) {
        existing.push(rel)
      }
    }

    for (const { fromLabel, relType, toLabel, relCount } of relCountRows) {
      const properties = relationshipPropertiesMap.get(`${fromLabel}|${relType}|${toLabel}`)

      if (fromLabel === toLabel) {
        // Self-referential edge (e.g. CHARACTER HAS_MENTOR_CHARACTER_ID CHARACTER): record it once as
        // outgoing so it stays traversable in the schema instead of being dropped.
        addRel(fromLabel, { label: toLabel, type: relType, direction: 'out', count: relCount, properties })
        continue
      }

      addRel(fromLabel, { label: toLabel, type: relType, direction: 'out', count: relCount, properties })
      addRel(toLabel, { label: fromLabel, type: relType, direction: 'in', count: relCount, properties })
    }

    // ---------- Merge into SchemaItem[] ----------
    // Include all labels from Q1 plus any additional labels seen in Q2/Q3
    const allLabels = new Set<string>([
      ...labelCountMap.keys(),
      ...labelPropsMap.keys(),
      ...labelRelsMap.keys()
    ])

    const schema: SchemaItem[] = []

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

      schema.push({
        label,
        count: labelCountMap.get(label) ?? 0,
        properties: labelPropsMap.get(label) ?? [],
        relationships: rels
      })
    }

    // ── Enrich properties with vector index info ─────────────────────────────
    if (allIndexes.length > 0) {
      // Build map: "label|propertyName" → index rows
      const indexMap = new Map<string, SchemaVectorIndex[]>()
      for (const idx of allIndexes) {
        const key = `${idx.label}|${idx.propertyName}`
        if (!indexMap.has(key)) {
          indexMap.set(key, [])
        }
        indexMap.get(key)!.push({
          id: idx.id,
          sourceType: idx.sourceType,
          similarityFunction: idx.similarityFunction,
          dimensions: idx.dimensions,
          status: idx.status,
          modelKey: idx.modelKey
        })
      }
      for (const item of schema) {
        for (const prop of item.properties) {
          const key = `${item.label}|${prop.name}`
          const indexes = indexMap.get(key)
          if (indexes?.length) {
            prop.vectorIndexes = indexes
          }
        }
      }
    }

    // Sort by count descending for consistent output
    schema.sort((a, b) => b.count - a.count)

    const elapsed = Date.now() - t0
    const propertiesCount = [...labelPropsMap.values()].reduce((sum, props) => sum + props.length, 0)

    isDevMode(() =>
      Logger.debug(
        `[AiService] schema: recalculated in ${elapsed}ms (${schema.length} labels, ${propertiesCount} properties, ${relCountRows.length} relationship triples)`
      )
    )

    // Emit COMPUTE_OPERATION only when a full recalculation was performed
    if (workspaceId) {
      this.kuEventsService.emit(workspaceId, projectId, KuOperation.COMPUTE_OPERATION, {
        type: 'schema'
      })
    }

    // ── Persist full schema to SQL projects table for subsequent cache hits ──
    const nowIso = new Date().toISOString()
    try {
      await this.projectRepository.update(projectId, {
        schemaCache: JSON.stringify(schema),
        schemaCachedAt: nowIso
      })
    } catch (err) {
      // Cache write is best-effort — log but never fail the caller
      console.warn('[AiService] failed to persist schema cache:', err)
    }

    return schema
  }

  buildMdSchema(schema: SchemaItem[]): string {
    const esc = (s: string) => String(s).replace(/\|/g, '\\|').replace(/`/g, '\\`')
    const MAX_DISPLAY_VALUES = 5
    const MAX_VALUE_LEN = 48 // truncate individual string values longer than this

    const truncate = (s: string): string =>
      s.length > MAX_VALUE_LEN ? s.slice(0, MAX_VALUE_LEN) + '\u2026' : s

    const fmtValues = (prop: SchemaProperty): string => {
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

    let md = `# Graph Schema\n\n`

    // Labels summary table
    md += `## Labels\n\n`
    md += `| Label | Count |\n|-------|------:|\n`
    for (const item of schema) {
      md += `| \`${esc(item.label)}\` | ${item.count} |\n`
    }
    md += `\n`

    // Per-label detail
    for (const item of schema) {
      const recordWord = item.count === 1 ? 'record' : 'records'
      md += `---\n\n## \`${esc(item.label)}\` (${item.count} ${recordWord})\n\n`

      if (item.properties.length > 0) {
        md += `### Properties\n\n`
        md += `| Property | Type | Values / Range | Semantic Search |\n|----------|------|----------------|-----------------|\n`
        for (const p of item.properties) {
          let semanticCell = '—'
          if (p.vectorIndexes?.length) {
            semanticCell = p.vectorIndexes
              .map((vi) => `\`${vi.sourceType}\` ${vi.similarityFunction} ${vi.dimensions}d [${vi.status}]`)
              .join(', ')
          }
          const propertyType = p.isArray ? `${p.type}[]` : p.type
          md += `| \`${esc(p.name)}\` | ${esc(propertyType)} | ${fmtValues(p)} | ${semanticCell} |\n`
        }
        md += `\n`
      }

      if (item.relationships.length > 0) {
        md += `### Relationships\n\n`
        // Each row is a directed traversal pattern rooted at this label: `->` is outgoing, `<-` is incoming.
        // Map straight to a where traversal: nest the other label and set $relation { type, direction } where
        // direction is "out" for `->` and "in" for `<-`.
        md += `| Traversal | Count | Edge Properties |\n|-----------|------:|-----------------|\n`
        for (const r of item.relationships) {
          const edgeProperties =
            r.properties?.length ?
              r.properties.map((p) => `\`${esc(p.name)}:${esc(p.type)}\``).join(', ')
            : '—'
          const pattern =
            r.direction === 'out' ?
              `(${item.label})-[:${r.type}]->(${r.label})`
            : `(${item.label})<-[:${r.type}]-(${r.label})`
          const count = typeof r.count === 'number' ? String(r.count) : '—'
          md += `| \`${esc(pattern)}\` | ${count} | ${edgeProperties} |\n`
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
      this.aiQueryService.getEmbeddingBackfillEstimateQuery(labelSuffix, vectorPropertyName),
      {
        projectId,
        propertyName,
        propKey,
        scanCap: neo4jInt(EMBEDDING_BILLING_SCAN_CAP)
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
      const probeKey = `${modelKey}:${dimensions}`
      const lastProbe = this.embeddingProbeCache.get(probeKey)
      if (!lastProbe || Date.now() - lastProbe > EMBEDDING_PROBE_TTL_MS) {
        try {
          const probe = await this.embeddingProviderService.embed('rushdb-embedding-healthcheck')
          if (!Array.isArray(probe) || probe.length !== dimensions) {
            throw new UnprocessableEntityException(
              `Embedding provider returned vector length ${probe?.length ?? 0}, expected ${dimensions}.`
            )
          }
          this.embeddingProbeCache.set(probeKey, Date.now())
        } catch (err) {
          if (err instanceof UnprocessableEntityException) {
            throw err
          }
          throw new UnprocessableEntityException(
            `Embedding provider validation failed for model "${modelKey}". ${err instanceof Error ? err.message : ''}`
          )
        }
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

    // 4. Create per-slot Neo4j vector index. DDL needs the exclusive schema lock and can
    // block for tens of seconds behind concurrent write transactions (backfill batches,
    // imports), so it must not hold up the HTTP response. The backfill scheduler retries
    // it for managed indexes; upsertIndexVectors retries it for external ones.
    void this.embeddingIndexDdlService
      .ensureVectorIndexExists({ sourceType, similarityFunction, dimensions })
      .catch((err) => {
        Logger.warn(`[AiService] vector index DDL failed for ${vectorIndexName} (will be retried): ${err}`)
      })

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
        await this.embeddingIndexDdlService.dropVectorIndex(vectorIndexName)
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

    // External indexes have no backfill, so this is their DDL retry path in case the
    // fire-and-forget creation at createIndex time failed. Cached, so usually a no-op.
    void this.embeddingIndexDdlService
      .ensureVectorIndexExists({
        sourceType: 'external',
        similarityFunction: row.similarityFunction as EmbeddingIndexSimilarityFunction,
        dimensions: row.dimensions
      })
      .catch((err) => {
        Logger.warn(`[AiService] vector index DDL failed for ${row.vectorPropertyName}: ${err}`)
      })

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
    const tTotal = Date.now()
    // Resolve the index by (projectId, propertyName, labels[0])
    const label = dto.labels[0]
    const sourceType: EmbeddingIndexSourceType = dto.sourceType ?? 'managed'
    const similarityFunction: EmbeddingIndexSimilarityFunction = dto.similarityFunction ?? 'cosine'
    const dimensions =
      dto.dimensions ??
      (Array.isArray(dto.queryVector) ? dto.queryVector.length : undefined) ??
      Number.parseInt(this.configService.get<string>('RUSHDB_EMBEDDING_DIMENSIONS') ?? '0', 10)

    const tIndex = Date.now()
    const index = await this.embeddingIndexRepository.findByProjectIdPropertyAndLabel(
      projectId,
      dto.propertyName,
      label,
      { sourceType, similarityFunction, dimensions }
    )
    const indexMs = Date.now() - tIndex
    if (!index) {
      throw new NotFoundException(
        `No embedding index found for label "${label}" and property "${dto.propertyName}" matching the requested vector signature`
      )
    }
    if (index.status === 'pending' || index.status === 'indexing') {
      isDevMode(() =>
        Logger.debug(
          `[AiService] semanticSearch: index for "${label}:${dto.propertyName}" is partially filled (status: ${index.status}). Results will only include already-indexed records.`
        )
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

      const tEmbedding = Date.now()
      const embeddingResult = await this.embeddingProviderService.embedWithUsage(dto.query)
      queryVector = embeddingResult.embedding
      tokensUsed = embeddingResult.tokensUsed
      isDevMode(() =>
        Logger.debug(`[AiService] semanticSearch timing: embedding=${Date.now() - tEmbedding}ms`)
      )
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

    let canUseVectorIndex = !hasWhere && !hasMultiLabels

    if (canUseVectorIndex) {
      const tDdl = Date.now()
      try {
        await this.embeddingIndexDdlService.ensureVectorIndexExists({
          sourceType: index.sourceType as EmbeddingIndexSourceType,
          similarityFunction: index.similarityFunction as EmbeddingIndexSimilarityFunction,
          dimensions: index.dimensions
        })
        isDevMode(() =>
          Logger.debug(`[AiService] semanticSearch timing: ensureVectorIndex=${Date.now() - tDdl}ms`)
        )
      } catch (err) {
        canUseVectorIndex = false
        const reason = err instanceof Error ? err.message : String(err)
        Logger.warn(
          `[AiService] semanticSearch: vector index unavailable for ${index.vectorPropertyName} after ${Date.now() - tDdl}ms; falling back to exact search: ${reason}`
        )
      }
    }

    if (canUseVectorIndex) {
      const candidateLimit = Math.min(Math.max(skip + limit, limit * 20, 1000), 10_000)
      query = this.aiQueryService.getSemanticSearchVectorIndexQuery({ labelSuffix })
      params = {
        queryVector,
        vectorIndexName: buildVectorIndexName({
          sourceType: index.sourceType as EmbeddingIndexSourceType,
          similarityFunction: index.similarityFunction as EmbeddingIndexSimilarityFunction,
          dimensions: index.dimensions
        }),
        candidateLimit: neo4jInt(candidateLimit),
        propertyName: dto.propertyName,
        propKey,
        projectId,
        skip: neo4jInt(skip),
        limit: neo4jInt(limit)
      }
    } else {
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

    const tCypher = Date.now()
    const result = await transaction.run(query, params)
    const cypherMs = Date.now() - tCypher

    const toNum = (v: any) => (typeof v?.toNumber === 'function' ? v.toNumber() : Number(v))

    const tMap = Date.now()
    const mapped = result.records.map((r) => {
      const record = r.get('record')
      const label =
        Array.isArray(record?.labels) ?
          record.labels.find((candidate: string) => candidate !== RUSHDB_LABEL_RECORD)
        : undefined

      return {
        ...(record?.properties ?? record),
        ...(label ? { [RUSHDB_KEY_LABEL_ALIAS]: label } : {}),
        __score: toNum(r.get('score'))
      }
    })
    const mapMs = Date.now() - tMap

    isDevMode(() =>
      Logger.debug(
        `[AiService] semanticSearch timing: total=${Date.now() - tTotal}ms indexLookup=${indexMs}ms cypher=${cypherMs}ms map=${mapMs}ms mode=${canUseVectorIndex ? 'vector-index' : 'exact'} rows=${mapped.length}`
      )
    )

    return mapped
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
