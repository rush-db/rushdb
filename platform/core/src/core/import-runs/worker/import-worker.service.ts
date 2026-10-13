import { Inject, Injectable, Logger, Optional } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Cron } from '@nestjs/schedule'

import { AiService } from '@/core/ai/ai.service'
import { BILLING_POLICY_PORT, type BillingPolicyPort } from '@/core/billing-policy/billing-policy.port'
import { RUSHDB_RELATION_DEFAULT } from '@/core/common/constants'
import { ImportService } from '@/core/entity/import-export/import.service'
import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { RelationshipPatternsService } from '@/core/relationship-patterns/relationship-patterns.service'
import { ProjectService } from '@/dashboard/project/project.service'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import { DbConnectionService } from '@/database/db-connection/db-connection.service'
import { NeogmaService } from '@/database/neogma/neogma.service'
import { DEFAULT_TRANSACTION_TIMEOUT_MS } from '@/database/transaction.constants'

import {
  IMPORT_ERROR_CODES,
  type ImportCheckpoint,
  type ImportFileFormat,
  type ImportLinkSpec
} from '../domain/import-run.types'
import { deriveDeterministicRecordId } from '../domain/replay-identity'
import { CsvImportParser } from '../parser/csv.parser'
import { JsonLinesImportParser } from '../parser/json-lines.parser'
import { JsonObjectImportParser } from '../parser/json-object.parser'
import { ParquetImportParser } from '../parser/parquet.parser'
import { ImportRunsRepository } from '../persistence/import-runs.repository'
import { IMPORT_STORAGE_PORT, type ImportStoragePort } from '../storage/import-storage.port'

import {
  createLinks,
  hasAnyRecordForLabel,
  linkRelations,
  mergeRecords,
  resolveEndpoints,
  type GraphTxLike,
  type RecordDraft,
  type RelationDraft
} from './import-batch-writer'

import type { ImportUnit } from '../parser/import-parser.port'
import type { TImportOptions } from '@/core/entity/import-export/import.types'
import type { ImportRunFileRow } from '@/database/sql/schema/types'
import type { Neogma } from 'neogma'

const BATCH_UNITS = 500
const LEASE_TTL_MS = 60_000
const ENDPOINT_RESOLUTION_CHUNK = 1_000

interface WorkerContext {
  projectId: string
  workspaceId: string | null
  connection: Neogma
}

interface BatchCounters {
  parsedUnits: number
  committedUnits: number
  recordsCommitted: number
  relationshipsCommitted: number
  linksResolved: number
  linksUnresolved: number
  skippedUnits: number
}

@Injectable()
export class ImportWorkerService {
  private running = false

  private readonly csvParser = new CsvImportParser()
  private readonly jsonLinesParser = new JsonLinesImportParser()
  private readonly jsonObjectParser = new JsonObjectImportParser()
  private readonly parquetParser = new ParquetImportParser()

  constructor(
    private readonly repository: ImportRunsRepository,
    @Inject(IMPORT_STORAGE_PORT) private readonly storage: ImportStoragePort,
    private readonly importService: ImportService,
    private readonly neogmaService: NeogmaService,
    private readonly workspaceService: WorkspaceService,
    private readonly kuEventsService: KuEventsService,
    @Inject(BILLING_POLICY_PORT) private readonly billingPolicy: BillingPolicyPort,
    private readonly projectService: ProjectService,
    private readonly dbConnectionService: DbConnectionService,
    private readonly configService: ConfigService,
    @Optional() private readonly relationshipPatternsService?: RelationshipPatternsService,
    @Optional() private readonly aiService?: AiService
  ) {}

  private parserFor(
    format: ImportFileFormat
  ): CsvImportParser | JsonLinesImportParser | JsonObjectImportParser | ParquetImportParser {
    switch (format) {
      case 'csv':
        return this.csvParser
      case 'jsonl':
      case 'ndjson':
        return this.jsonLinesParser
      case 'json':
        return this.jsonObjectParser
      case 'parquet':
        return this.parquetParser
      default:
        throw new Error(`${IMPORT_ERROR_CODES.FORMAT_UNSUPPORTED}: ${format}`)
    }
  }

  private maxSourceBytesFor(format: ImportFileFormat): number | undefined {
    if (format === 'json') {
      const mb =
        Number(this.configService.get<string>('RUSHDB_IMPORT_MAX_JSON_BYTES', '')) || 64 * 1024 * 1024
      return mb
    }
    if (format === 'parquet') {
      const mb =
        Number(this.configService.get<string>('RUSHDB_IMPORT_MAX_PARQUET_BYTES', '')) || 256 * 1024 * 1024
      return mb
    }
    return undefined
  }

  @Cron('*/5 * * * * *')
  async poll(): Promise<void> {
    if (this.running) {
      return
    }
    this.running = true
    try {
      for (const projectId of await this.repository.listClaimableProjects()) {
        const claimed = await this.repository.claimNextFile({
          projectId,
          workerId: `worker-${process.pid}`,
          now: new Date().toISOString(),
          leaseUntil: new Date(Date.now() + LEASE_TTL_MS).toISOString()
        })
        if (claimed) {
          await this.processClaimed(claimed)
        }
      }
    } catch (error) {
      Logger.error('[ImportWorker] poll failed', error)
    } finally {
      this.running = false
    }
  }

  async processClaimed(file: ImportRunFileRow): Promise<void> {
    try {
      const context = await this.resolveContext(file.projectId)
      await this.preflightBilling(file, context)
      if (file.role === 'links') {
        await this.processLinksFile(file, context)
      } else {
        await this.processRecordsFile(file, context)
      }
    } catch (error) {
      await this.handleProcessingError(file, error as Error)
    }
  }

  // ------------------------------------------------------------------
  // Records role
  // ------------------------------------------------------------------

  private async processRecordsFile(file: ImportRunFileRow, context: WorkerContext): Promise<void> {
    const rootLabel = requireRootLabel(file)
    const options = parseJson<TImportOptions>(file.importOptions) ?? {}
    const checkpoint = readCheckpoint(file)

    await this.repository.fencedUpdateFile(file.id, file.leaseGeneration, {
      status: 'running',
      stage: 'parse',
      heartbeatAt: new Date().toISOString()
    })

    const stream = await this.openSourceStream(file)
    const format = file.format as ImportFileFormat
    const parser = this.parserFor(format)
    const units = await parser.parse(stream, {
      maxSourceBytes: this.maxSourceBytesFor(format),
      signal: undefined
    })

    let batchOrdinal = checkpoint.lastCommittedBatch + 1
    let buffer: ImportUnit[] = []
    let counters = readCounters(file)

    for await (const unit of units) {
      if (unit.ordinal < checkpoint.nextUnit) {
        continue
      }

      buffer.push(unit)
      counters.parsedUnits += 1

      if (buffer.length >= BATCH_UNITS) {
        counters = await this.commitRecordsBatch(
          file,
          context,
          buffer,
          rootLabel,
          options,
          batchOrdinal,
          counters
        )
        batchOrdinal += 1
        buffer = []

        if (!(await this.continueProcessing(file))) {
          return
        }
      }
    }

    if (buffer.length > 0) {
      counters = await this.commitRecordsBatch(
        file,
        context,
        buffer,
        rootLabel,
        options,
        batchOrdinal,
        counters
      )
    }

    await this.completeFile(file, counters)
  }

  private async commitRecordsBatch(
    file: ImportRunFileRow,
    context: WorkerContext,
    units: ImportUnit[],
    rootLabel: string,
    options: TImportOptions,
    batchOrdinal: number,
    runningCounters: BatchCounters
  ): Promise<BatchCounters> {
    const wantsUpsert =
      Boolean(options.mergeStrategy) || (Array.isArray(options.mergeBy) && options.mergeBy.length > 0)

    let recordsCommitted = 0
    let relationshipsCommitted = 0

    if (wantsUpsert) {
      // mergeBy/mergeStrategy: business upsert is naturally idempotent, so route
      // through the shared ImportService on this worker's write transaction. It
      // handles records, relations, vectors and KU emission internally.
      await this.withWriteTransaction(context.connection, async (tx) => {
        const result = await this.importService.importRecords(
          {
            data: units.map((u) => u.value),
            label: rootLabel,
            options: { ...options, returnResult: false }
          },
          file.projectId,
          tx as never,
          tx as never
        )
        if (typeof result === 'object' && result && 'count' in result) {
          recordsCommitted = Number((result as { count: number }).count)
        }
      })
    } else {
      const records: RecordDraft[] = []
      const relations: RelationDraft[] = []
      const vectorDrafts: Array<{ deterministicId: string; label: string; vectors: unknown[] }> = []

      for (const unit of units) {
        const [drafts, unitRelations, unitVectors] = this.importService.serializeBFS(unit.value, rootLabel, {
          ...options,
          returnResult: false
        })

        const idRemap = new Map<string, string>()
        drafts.forEach((draft, index) => {
          const deterministicId = deriveDeterministicRecordId({
            projectId: file.projectId,
            fileId: file.id,
            sourceGeneration: file.sourceGeneration,
            unitOrdinal: unit.ordinal,
            path: String(index),
            siblingOccurrence: 0
          })
          idRemap.set(draft.id, deterministicId)
          records.push({
            id: deterministicId,
            label: draft.label || rootLabel,
            properties: draft.properties
          })
        })

        for (const vd of unitVectors) {
          const deterministicId = idRemap.get(vd.draftId)
          if (deterministicId) {
            vectorDrafts.push({ deterministicId, label: vd.label, vectors: vd.vectors })
          }
        }

        for (const relation of unitRelations) {
          const sourceId = idRemap.get(relation.source)
          const targetId = idRemap.get(relation.target)
          if (sourceId && targetId) {
            relations.push({
              source: sourceId,
              target: targetId,
              type: relation.type ?? RUSHDB_RELATION_DEFAULT
            })
          }
        }
      }

      await this.withWriteTransaction(context.connection, async (tx) => {
        recordsCommitted = await mergeRecords(tx, { projectId: file.projectId, records })
        relationshipsCommitted = await linkRelations(tx, { projectId: file.projectId, relations })
        if (this.aiService) {
          for (const vd of vectorDrafts) {
            await this.aiService
              .resolveAndWriteInlineVectors(
                file.projectId,
                vd.label,
                vd.deterministicId,
                vd.vectors as never,
                tx as never
              )
              .catch(() => undefined)
          }
        }
      })

      await this.emitKu(file.projectId, context.workspaceId, recordsCommitted, relationshipsCommitted)
    }

    const nextCounters: BatchCounters = {
      parsedUnits: runningCounters.parsedUnits,
      committedUnits: runningCounters.committedUnits + units.length,
      recordsCommitted: runningCounters.recordsCommitted + recordsCommitted,
      relationshipsCommitted: runningCounters.relationshipsCommitted + relationshipsCommitted,
      linksResolved: runningCounters.linksResolved,
      linksUnresolved: runningCounters.linksUnresolved,
      skippedUnits: runningCounters.skippedUnits
    }

    const persisted = await this.persistProgress(file, batchOrdinal, nextCounters)
    if (!persisted) {
      throw new Error(`${IMPORT_ERROR_CODES.LEASE_LOST}: cannot checkpoint batch ${batchOrdinal}`)
    }

    return nextCounters
  }

  // ------------------------------------------------------------------
  // Links role
  // ------------------------------------------------------------------

  private async processLinksFile(file: ImportRunFileRow, context: WorkerContext): Promise<void> {
    const spec = parseJson<ImportLinkSpec>(file.linkSpec)
    assertLinkSpec(spec)

    const checkpoint = readCheckpoint(file)

    await this.repository.fencedUpdateFile(file.id, file.leaseGeneration, {
      status: 'running',
      stage: 'write',
      heartbeatAt: new Date().toISOString()
    })

    await this.assertLinkLabelsExist(file, spec, context.connection)

    const stream = await this.openSourceStream(file)
    const format = file.format as ImportFileFormat
    const parser = this.parserFor(format)
    const units = await parser.parse(stream, {
      maxSourceBytes: this.maxSourceBytesFor(format),
      signal: undefined
    })
    const importOptions = parseJson<Record<string, unknown>>(file.importOptions) ?? {}
    const skipInvalidRows = importOptions.skipInvalidRows === true

    let batchOrdinal = checkpoint.lastCommittedBatch + 1
    let buffer: ImportUnit[] = []
    let counters = readCounters(file)

    for await (const unit of units) {
      if (unit.ordinal < checkpoint.nextUnit) {
        continue
      }

      buffer.push(unit)
      counters.parsedUnits += 1

      if (buffer.length >= BATCH_UNITS) {
        counters = await this.commitLinksBatch(
          file,
          context,
          spec,
          buffer,
          batchOrdinal,
          counters,
          skipInvalidRows
        )
        batchOrdinal += 1
        buffer = []

        if (!(await this.continueProcessing(file))) {
          return
        }
      }
    }

    if (buffer.length > 0) {
      counters = await this.commitLinksBatch(
        file,
        context,
        spec,
        buffer,
        batchOrdinal,
        counters,
        skipInvalidRows
      )
    }

    await this.completeFile(file, counters)
  }

  private async commitLinksBatch(
    file: ImportRunFileRow,
    context: WorkerContext,
    spec: ImportLinkSpec,
    units: ImportUnit[],
    batchOrdinal: number,
    runningCounters: BatchCounters,
    skipInvalidRows: boolean
  ): Promise<BatchCounters> {
    const [sourceEndpoint, targetEndpoint] = spec.endpoints
    const propertyColumns = spec.propertyColumns ?? {}

    interface PendingRow {
      row: Record<string, unknown>
      properties: Record<string, unknown>
    }

    const pending: PendingRow[] = units.map((unit) => {
      const properties: Record<string, unknown> = {}
      for (const [column, propertyName] of Object.entries(propertyColumns)) {
        properties[propertyName] = unit.value[column]
      }
      return { row: unit.value, properties }
    })

    const sourceValues = uniqueStrings(
      pending.map((entry) => stringifyValue(entry.row[sourceEndpoint.column]))
    )
    const targetValues = uniqueStrings(
      pending.map((entry) => stringifyValue(entry.row[targetEndpoint.column]))
    )

    let linksResolved = 0
    let linksCreated = 0

    await this.withWriteTransaction(context.connection, async (tx) => {
      const resolvedSources = await this.resolveInChunks(tx, file.projectId, sourceEndpoint, sourceValues)
      const resolvedTargets = await this.resolveInChunks(tx, file.projectId, targetEndpoint, targetValues)

      const pairs = pending.map((entry) => {
        const sourceValue = stringifyValue(entry.row[sourceEndpoint.column])
        const targetValue = stringifyValue(entry.row[targetEndpoint.column])
        return {
          sourceValue,
          targetValue,
          sourceId: resolvedSources.get(sourceValue),
          targetId: resolvedTargets.get(targetValue),
          properties: entry.properties
        }
      })

      const unresolvedPairs = pairs.filter((pair) => !pair.sourceId || !pair.targetId)

      for (const unresolved of unresolvedPairs.slice(0, 5)) {
        await this.repository.addErrorSample({
          runId: file.runId,
          fileId: file.id,
          projectId: file.projectId,
          code: IMPORT_ERROR_CODES.LINK_ENDPOINT_UNRESOLVED,
          message: sanitizeSampleMessage(
            `unresolved endpoint linking ${unresolved.sourceValue} -> ${unresolved.targetValue}`
          )
        })
      }

      if (unresolvedPairs.length > 0 && !skipInvalidRows) {
        throw new Error(
          `${IMPORT_ERROR_CODES.LINK_ENDPOINT_UNRESOLVED}: ${unresolvedPairs.length} row(s) reference missing records`
        )
      }

      const result = await createLinks(tx, {
        projectId: file.projectId,
        relationshipType: spec.relationshipType,
        pairs
      })

      linksResolved = pairs.length - unresolvedPairs.length
      linksCreated = result.created
    })

    await this.emitKu(file.projectId, context.workspaceId, 0, linksCreated)

    const nextCounters: BatchCounters = {
      parsedUnits: runningCounters.parsedUnits,
      committedUnits: runningCounters.committedUnits + units.length,
      recordsCommitted: runningCounters.recordsCommitted,
      relationshipsCommitted: runningCounters.relationshipsCommitted + linksCreated,
      linksResolved: runningCounters.linksResolved + linksResolved,
      linksUnresolved: runningCounters.linksUnresolved + (units.length - linksResolved),
      skippedUnits: runningCounters.skippedUnits
    }

    const persisted = await this.persistProgress(file, batchOrdinal, nextCounters)
    if (!persisted) {
      throw new Error(`${IMPORT_ERROR_CODES.LEASE_LOST}: cannot checkpoint batch ${batchOrdinal}`)
    }

    return nextCounters
  }

  private async resolveInChunks(
    tx: GraphTxLike,
    projectId: string,
    endpoint: { label: string; keyProperty: string },
    values: string[]
  ): Promise<Map<string, string>> {
    const resolved = new Map<string, string>()
    for (let i = 0; i < values.length; i += ENDPOINT_RESOLUTION_CHUNK) {
      const chunk = values.slice(i, i + ENDPOINT_RESOLUTION_CHUNK)
      const part = await resolveEndpoints(tx, {
        projectId,
        label: endpoint.label,
        keyProperty: endpoint.keyProperty,
        values: chunk
      })
      for (const [value, id] of part) {
        resolved.set(value, id)
      }
    }
    return resolved
  }

  private async assertLinkLabelsExist(
    file: ImportRunFileRow,
    spec: ImportLinkSpec,
    connection: Neogma
  ): Promise<void> {
    await this.withWriteTransaction(connection, async (tx) => {
      for (const endpoint of spec.endpoints) {
        const exists = await hasAnyRecordForLabel(tx, {
          projectId: file.projectId,
          label: endpoint.label
        })
        if (!exists) {
          throw new Error(
            `${IMPORT_ERROR_CODES.LINK_LABEL_EMPTY}: no ${endpoint.label} records exist to link against`
          )
        }
      }
    })
  }

  // ------------------------------------------------------------------
  // Shared plumbing
  // ------------------------------------------------------------------

  private async openSourceStream(file: ImportRunFileRow): Promise<NodeJS.ReadableStream> {
    if (!file.storageKey) {
      throw new Error(`${IMPORT_ERROR_CODES.SOURCE_MISSING}: no stored source for file`)
    }
    const meta = await this.storage.head(file.storageKey)
    if (!meta) {
      throw new Error(`${IMPORT_ERROR_CODES.SOURCE_MISSING}: object not found`)
    }
    return this.storage.readStream(file.storageKey)
  }

  private async continueProcessing(file: ImportRunFileRow): Promise<boolean> {
    const fresh = await this.repository.getFile(file.id, file.projectId)
    if (!fresh) {
      return false
    }
    if (fresh.status === 'canceled' || fresh.cancelRequestedAt || fresh.status !== 'running') {
      return false
    }
    return true
  }

  private async persistProgress(
    file: ImportRunFileRow,
    batchOrdinal: number,
    counters: BatchCounters
  ): Promise<boolean> {
    const checkpoint: ImportCheckpoint = {
      version: 1,
      sourceGeneration: file.sourceGeneration,
      nextUnit: counters.parsedUnits,
      lastCommittedBatch: batchOrdinal
    }

    return this.repository.fencedUpdateFile(file.id, file.leaseGeneration, {
      status: 'running',
      stage: 'write',
      currentBatch: batchOrdinal,
      checkpoint: JSON.stringify(checkpoint),
      heartbeatAt: new Date().toISOString(),
      leaseUntil: new Date(Date.now() + LEASE_TTL_MS).toISOString(),
      parsedUnits: counters.parsedUnits,
      committedUnits: counters.committedUnits,
      recordsCommitted: counters.recordsCommitted,
      relationshipsCommitted: counters.relationshipsCommitted,
      linksResolved: counters.linksResolved,
      linksUnresolved: counters.linksUnresolved,
      skippedUnits: counters.skippedUnits
    })
  }

  private async completeFile(file: ImportRunFileRow, counters: BatchCounters): Promise<void> {
    const checkpoint: ImportCheckpoint = {
      version: 1,
      sourceGeneration: file.sourceGeneration,
      nextUnit: counters.parsedUnits,
      lastCommittedBatch: Math.max(0, counters.committedUnits - 1)
    }

    const persisted = await this.repository.fencedUpdateFile(file.id, file.leaseGeneration, {
      status: 'completed',
      stage: 'finalize',
      finishedAt: new Date().toISOString(),
      checkpoint: JSON.stringify(checkpoint),
      parsedUnits: counters.parsedUnits,
      committedUnits: counters.committedUnits,
      recordsCommitted: counters.recordsCommitted,
      relationshipsCommitted: counters.relationshipsCommitted,
      linksResolved: counters.linksResolved,
      linksUnresolved: counters.linksUnresolved,
      skippedUnits: counters.skippedUnits
    })

    if (!persisted) {
      throw new Error(`${IMPORT_ERROR_CODES.LEASE_LOST}: cannot mark completed`)
    }

    await this.repository.addEvent({
      runId: file.runId,
      fileId: file.id,
      projectId: file.projectId,
      type: 'FILE_COMPLETED',
      toStatus: 'completed'
    })

    await this.maybeFinalizeRun(file.runId)
  }

  private async handleProcessingError(file: ImportRunFileRow, error: Error): Promise<void> {
    const message = error.message ?? String(error)

    if (message.includes(IMPORT_ERROR_CODES.LEASE_LOST)) {
      Logger.warn(`[ImportWorker] lost lease on file ${file.id}`)
      return
    }

    const code = extractCode(message)
    const fresh = await this.repository.getFile(file.id, file.projectId)
    if (!fresh) {
      return
    }

    if (code) {
      await this.failFile(fresh, code, message)
      return
    }

    const nextAttempt = fresh.attemptCount
    if (nextAttempt >= fresh.maxAttempts) {
      await this.failFile(fresh, IMPORT_ERROR_CODES.INTERNAL, message)
      return
    }

    const backoffMs = Math.min(60_000, 2 ** nextAttempt * 1_000)
    await this.repository.updateFile(fresh.id, {
      status: 'retry_wait',
      notBefore: new Date(Date.now() + backoffMs).toISOString(),
      lastErrorMessage: sanitizeSampleMessage(message)
    })
  }

  private async failFile(file: ImportRunFileRow, code: string, message: string): Promise<void> {
    await this.repository.updateFile(file.id, {
      status: 'failed',
      finishedAt: new Date().toISOString(),
      lastErrorCode: code,
      lastErrorMessage: sanitizeSampleMessage(message)
    })
    await this.repository.addEvent({
      runId: file.runId,
      fileId: file.id,
      projectId: file.projectId,
      type: 'FILE_FAILED',
      code,
      message: sanitizeSampleMessage(message)
    })
    await this.maybeFinalizeRun(file.runId)
  }

  /**
   * Run finalization: when every file of a run is terminal, refresh aggregates,
   * execute the post-write side effects against committed data (recount ->
   * approved pattern application -> schema cache refresh -> analysis enqueue),
   * then stamp the run outcome.
   */
  private async maybeFinalizeRun(runId: string): Promise<void> {
    const files = await this.repository.listFiles(runId)
    const allTerminal = files.every((f) => ['completed', 'failed', 'canceled'].includes(f.status))
    if (!allTerminal) {
      return
    }

    const run = await this.repository.refreshRunAggregates(runId)
    if (!run) {
      return
    }

    const outcome = deriveOutcome(files.map((f) => f.status))

    try {
      await this.runPostWriteSideEffects(run.projectId)
    } catch (error) {
      Logger.error(`[ImportWorker] finalization side effects failed for run ${runId}`, error)
      await this.repository.updateRun(runId, { status: 'completed_with_errors' })
      return
    }

    const retentionHours = Number(this.configService.get<string>('RUSHDB_IMPORT_RETENTION_HOURS', '72'))
    await this.repository.updateRun(runId, {
      status: outcome,
      finalizedAt: new Date().toISOString(),
      retentionUntil: new Date(Date.now() + Math.max(1, retentionHours) * 3_600_000).toISOString()
    })
  }

  /** Mirrors RunSideEffectMixin ordering: recount -> apply patterns -> schema -> analysis. */
  private async runPostWriteSideEffects(projectId: string): Promise<void> {
    const session = this.neogmaService.createSession('import-runs-side-effect')
    const transaction = session.beginTransaction({ timeout: DEFAULT_TRANSACTION_TIMEOUT_MS })

    try {
      try {
        await this.projectService.recomputeProjectNodes(projectId, transaction)
      } catch (error) {
        Logger.error(`[ImportWorker] recount ERROR: project ${projectId}`, error)
      }

      if (this.relationshipPatternsService) {
        try {
          await this.relationshipPatternsService.applyApprovedPatterns(projectId, transaction)
        } catch (error) {
          Logger.error(`[ImportWorker] relationship apply ERROR: project ${projectId}`, error)
        }
      }

      if (transaction.isOpen()) {
        await transaction.commit()
      }
    } catch (error) {
      if (transaction.isOpen()) {
        await transaction.rollback().catch(() => undefined)
      }
      throw error
    } finally {
      try {
        await this.neogmaService.closeSession(session, 'import-runs')
      } catch {
        /* empty */
      }
    }

    if (this.aiService) {
      try {
        await this.aiService.getSchema({ projectId, force: true })
      } catch (error) {
        Logger.error(`[ImportWorker] schema recompute ERROR: project ${projectId}`, error)
      }
    }

    if (this.relationshipPatternsService) {
      try {
        await this.relationshipPatternsService.markAfterWrite(projectId)
      } catch (error) {
        Logger.error(`[ImportWorker] relationship analysis ERROR: project ${projectId}`, error)
      }
    }
  }

  private async resolveContext(projectId: string): Promise<WorkerContext> {
    let workspaceId: string | null = null
    try {
      const workspace = await this.workspaceService.getWorkspaceByProject(projectId)
      workspaceId = workspace?.id ?? null
    } catch {
      /* workspace lookup is best-effort for billing */
    }

    let connection: Neogma
    try {
      const project = await this.projectService.getProjectById(projectId)
      const result = await this.dbConnectionService.getConnection(projectId, project as never)
      connection = result.connection
    } catch {
      connection = this.neogmaService.getInstance()
    }

    return { projectId, workspaceId, connection }
  }

  private async preflightBilling(file: ImportRunFileRow, context: WorkerContext): Promise<void> {
    if (!context.workspaceId) {
      return
    }
    const estimatedKu = Math.max(10, Math.ceil((file.declaredSizeBytes ?? 0) / 1024))
    try {
      await this.billingPolicy.assertProjectOperationAllowed(context.workspaceId, { estimatedKu })
    } catch (error) {
      throw new Error(`${IMPORT_ERROR_CODES.QUOTA_BLOCKED}: ${(error as Error).message}`)
    }
  }

  private async emitKu(
    projectId: string,
    workspaceId: string | null,
    records: number,
    relationships: number
  ): Promise<void> {
    if (!workspaceId) {
      return
    }
    if (records > 0) {
      this.kuEventsService.emitBulk(workspaceId, projectId, KuOperation.ENTITY_CREATED, records)
    }
    if (relationships > 0) {
      this.kuEventsService.emitBulk(workspaceId, projectId, KuOperation.RELATIONSHIP_CREATED, relationships)
    }
  }

  private async withWriteTransaction(
    connection: Neogma,
    fn: (tx: GraphTxLike) => Promise<void>
  ): Promise<void> {
    const session = connection?.driver?.session() ?? this.neogmaService.createSession('import-runs-write')
    const tx = session.beginTransaction({ timeout: DEFAULT_TRANSACTION_TIMEOUT_MS })
    try {
      await fn(tx as unknown as GraphTxLike)
      await tx.commit()
    } catch (error) {
      if (tx.isOpen()) {
        await tx.rollback().catch(() => undefined)
      }
      throw error
    } finally {
      try {
        await session.close()
      } catch {
        /* empty */
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Module-level pure helpers
// ---------------------------------------------------------------------------

function parseJson<T>(raw: string | null | undefined): T | null {
  if (!raw) {
    return null
  }
  try {
    return JSON.parse(raw) as T
  } catch {
    return null
  }
}

function deriveOutcome(statuses: string[]): 'completed' | 'completed_with_errors' | 'failed' | 'canceled' {
  if (statuses.every((s) => s === 'canceled')) {
    return 'canceled'
  }
  if (statuses.includes('completed')) {
    return statuses.some((s) => s !== 'completed') ? 'completed_with_errors' : 'completed'
  }
  if (statuses.some((s) => s === 'completed')) {
    return 'completed_with_errors'
  }
  return 'failed'
}

function requireRootLabel(file: ImportRunFileRow): string {
  if (!file.rootLabel) {
    throw new Error(`${IMPORT_ERROR_CODES.LABEL_INVALID}: rootLabel required for records files`)
  }
  return file.rootLabel
}

function assertLinkSpec(spec: ImportLinkSpec | null): asserts spec is ImportLinkSpec {
  if (!spec || !Array.isArray(spec.endpoints) || spec.endpoints.length !== 2) {
    throw new Error(`${IMPORT_ERROR_CODES.LINK_SPEC_INVALID}: link spec missing or malformed`)
  }
}

export function readCheckpoint(file: ImportRunFileRow): ImportCheckpoint {
  if (!file.checkpoint) {
    return { version: 1, sourceGeneration: file.sourceGeneration, nextUnit: 0, lastCommittedBatch: -1 }
  }
  try {
    const parsed = JSON.parse(file.checkpoint) as ImportCheckpoint
    if (parsed.version !== 1 || typeof parsed.nextUnit !== 'number') {
      throw new Error('unsupported checkpoint shape')
    }
    return parsed
  } catch {
    throw new Error(`${IMPORT_ERROR_CODES.INTERNAL}: unreadable checkpoint`)
  }
}

function readCounters(file: ImportRunFileRow): BatchCounters {
  return {
    parsedUnits: file.parsedUnits ?? 0,
    committedUnits: file.committedUnits ?? 0,
    recordsCommitted: file.recordsCommitted ?? 0,
    relationshipsCommitted: file.relationshipsCommitted ?? 0,
    linksResolved: file.linksResolved ?? 0,
    linksUnresolved: file.linksUnresolved ?? 0,
    skippedUnits: file.skippedUnits ?? 0
  }
}

function extractCode(message: string): string | null {
  const match = /^([A-Z][A-Z0-9_]+):/.exec(message.trim())
  return match ? match[1] : null
}

function stringifyValue(value: unknown): string {
  if (value === null || value === undefined) {
    return ''
  }
  return String(value)
}

function uniqueStrings(values: string[]): string[] {
  return Array.from(new Set(values))
}

function sanitizeSampleMessage(message: string): string {
  let sanitized = ''
  for (const char of message.slice(0, 500)) {
    const code = char.charCodeAt(0)
    if (code > 0x1f || code === 0x09) {
      sanitized += char
    }
  }
  return sanitized
}
