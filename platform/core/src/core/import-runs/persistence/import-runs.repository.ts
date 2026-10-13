import { Injectable } from '@nestjs/common'
import { and, asc, desc, eq, gt, inArray, isNotNull, isNull, lte, ne, or, sql } from 'drizzle-orm'
import { uuidv7 } from 'uuidv7'

import { SqlService } from '@/database/sql/sql.service'

import { RECORDS_PHASE_TERMINAL_STATUSES, type ImportFileStatus } from '../domain/import-run.types'

import type {
  ImportErrorSampleRow,
  ImportRunEventRow,
  ImportRunFileRow,
  ImportRunRow,
  InsertImportErrorSampleRow,
  InsertImportRunEventRow,
  InsertImportRunFileRow,
  InsertImportRunRow
} from '@/database/sql/schema/types'

export interface ClaimInput {
  projectId: string
  workerId: string
  now: string
  leaseUntil: string
}

const MAX_ERROR_SAMPLES_PER_FILE = 20

@Injectable()
export class ImportRunsRepository {
  constructor(private readonly sql: SqlService) {}

  private get db() {
    return this.sql.db
  }

  private get runs() {
    return this.sql.tables.importRuns
  }

  private get files() {
    return this.sql.tables.importRunFiles
  }

  private get events() {
    return this.sql.tables.importRunEvents
  }

  private get errorSamples() {
    return this.sql.tables.importErrorSamples
  }

  // ---------- runs ----------

  async createRun(data: Omit<InsertImportRunRow, 'id' | 'createdAt' | 'updatedAt'>): Promise<ImportRunRow> {
    const now = new Date().toISOString()
    const row: InsertImportRunRow = { ...data, id: uuidv7(), createdAt: now, updatedAt: now }
    await this.db.insert(this.runs).values(row)
    return this.getRun(row.id as string) as Promise<ImportRunRow>
  }

  async getRun(id: string, projectId?: string): Promise<ImportRunRow | undefined> {
    const where =
      projectId ? and(eq(this.runs.id, id), eq(this.runs.projectId, projectId)) : eq(this.runs.id, id)
    const rows = await this.db.select().from(this.runs).where(where)
    return rows[0]
  }

  async getRunByIdempotencyKey(
    projectId: string,
    idempotencyKeyHash: string
  ): Promise<ImportRunRow | undefined> {
    const rows = await this.db
      .select()
      .from(this.runs)
      .where(and(eq(this.runs.projectId, projectId), eq(this.runs.idempotencyKeyHash, idempotencyKeyHash)))
    return rows[0]
  }

  async listRuns(projectId: string, limit = 50): Promise<ImportRunRow[]> {
    return this.db
      .select()
      .from(this.runs)
      .where(eq(this.runs.projectId, projectId))
      .orderBy(desc(this.runs.createdAt))
      .limit(limit)
  }

  async updateRun(
    id: string,
    data: Partial<Omit<InsertImportRunRow, 'id' | 'projectId' | 'createdAt'>>
  ): Promise<void> {
    await this.db
      .update(this.runs)
      .set({ ...data, updatedAt: new Date().toISOString() })
      .where(eq(this.runs.id, id))
  }

  async deleteDraftRun(id: string, projectId: string): Promise<void> {
    await this.db.delete(this.runs).where(and(eq(this.runs.id, id), eq(this.runs.projectId, projectId)))
  }

  // ---------- files ----------

  async createFiles(
    items: Omit<InsertImportRunFileRow, 'id' | 'createdAt' | 'updatedAt'>[]
  ): Promise<ImportRunFileRow[]> {
    const now = new Date().toISOString()
    const rows: InsertImportRunFileRow[] = items.map((item) => ({
      ...item,
      id: uuidv7(),
      createdAt: now,
      updatedAt: now
    }))
    await this.db.insert(this.files).values(rows)
    return this.db
      .select()
      .from(this.files)
      .where(eq(this.files.runId, rows[0].runId as string))
  }

  async getFile(id: string, projectId: string): Promise<ImportRunFileRow | undefined> {
    const rows = await this.db
      .select()
      .from(this.files)
      .where(and(eq(this.files.id, id), eq(this.files.projectId, projectId)))
    return rows[0]
  }

  async listFiles(runId: string): Promise<ImportRunFileRow[]> {
    return this.db
      .select()
      .from(this.files)
      .where(eq(this.files.runId, runId))
      .orderBy(asc(this.files.ordinal))
  }

  async updateFile(
    id: string,
    data: Partial<Omit<InsertImportRunFileRow, 'id' | 'runId' | 'projectId' | 'createdAt'>>,
    projectId?: string
  ): Promise<ImportRunFileRow | undefined> {
    const where =
      projectId ? and(eq(this.files.id, id), eq(this.files.projectId, projectId)) : eq(this.files.id, id)
    await this.db
      .update(this.files)
      .set({ ...data, updatedAt: new Date().toISOString() })
      .where(where)
    const rows = await this.db.select().from(this.files).where(eq(this.files.id, id))
    return rows[0]
  }

  /**
   * Fenced progress write: only succeeds while the caller still owns the lease
   * generation and the file remains in an active state.
   */
  async fencedUpdateFile(
    id: string,
    leaseGeneration: number,
    data: Partial<Omit<InsertImportRunFileRow, 'id' | 'createdAt'>>
  ): Promise<boolean> {
    const predicate = and(
      eq(this.files.id, id),
      eq(this.files.leaseGeneration, leaseGeneration),
      inArray(this.files.status, ['queued', 'validating', 'running', 'finalizing'])
    )
    const result = await this.db
      .update(this.files)
      .set({ ...data, updatedAt: new Date().toISOString() })
      .where(predicate)
    return this.extractRowCount(result) > 0
  }

  async countNonTerminalRecordsSiblings(runId: string, excludeFileId?: string): Promise<number> {
    const conditions = [
      eq(this.files.runId, runId),
      eq(this.files.role, 'records'),
      sql`${this.files.status} NOT IN (${sql.join(
        RECORDS_PHASE_TERMINAL_STATUSES.map((status) => sql`${status}`),
        sql`, `
      )})`
    ]
    if (excludeFileId) {
      conditions.push(ne(this.files.id, excludeFileId))
    }
    const rows = await this.db
      .select({ count: sql<number>`count(*)` })
      .from(this.files)
      .where(and(...conditions))
    return Number(rows[0]?.count ?? 0)
  }

  /**
   * Atomic, lease-fenced claim of the oldest eligible file for a project.
   *
   * PostgreSQL uses UPDATE .. WHERE id = (SELECT .. FOR UPDATE SKIP LOCKED) so two
   * replicas can never claim the same row. SQLite runs in enforced single-process
   * mode, so a conditional optimistic update provides the same guarantee.
   *
   * Eligibility includes the links-phase gate: a role=links file is claimable only
   * when every role=records sibling of its run has reached a terminal state.
   */
  async claimNextFile(input: ClaimInput): Promise<ImportRunFileRow | undefined> {
    if (this.sql.isPostgres) {
      return this.claimNextFilePostgres(input)
    }
    return this.claimNextFileSqlite(input)
  }

  private terminalStatusesSql() {
    return sql.join(
      RECORDS_PHASE_TERMINAL_STATUSES.map((s) => sql`${s}`),
      sql`, `
    )
  }

  private async claimNextFilePostgres(input: ClaimInput): Promise<ImportRunFileRow | undefined> {
    const result = await this.db.execute(sql`
      UPDATE import_run_files AS f SET
        status = 'validating',
        stage = 'validate',
        lease_owner = ${input.workerId},
        lease_generation = f.lease_generation + 1,
        lease_until = ${input.leaseUntil},
        heartbeat_at = ${input.now},
        attempt_count = f.attempt_count + 1,
        started_at = COALESCE(f.started_at, ${input.now}),
        updated_at = ${input.now}
      WHERE f.id = (
        SELECT c.id FROM import_run_files c
        JOIN import_runs r ON r.id = c.run_id
        WHERE c.project_id = ${input.projectId}
          AND c.status IN ('queued', 'retry_wait')
          AND (c.not_before IS NULL OR c.not_before <= ${input.now})
          AND (c.lease_until IS NULL OR c.lease_until < ${input.now})
          AND r.cancel_requested_at IS NULL
          AND (
            SELECT COUNT(*) FROM import_run_files active
            WHERE active.project_id = ${input.projectId}
              AND active.status IN ('validating', 'running')
          ) = 0
          AND (
            c.role = 'records'
            OR NOT EXISTS (
              SELECT 1 FROM import_run_files pending_records
              WHERE pending_records.run_id = c.run_id
                AND pending_records.role = 'records'
                AND pending_records.status NOT IN (${this.terminalStatusesSql()})
            )
          )
        ORDER BY c.created_at
        FOR UPDATE OF c SKIP LOCKED
        LIMIT 1
      )
      RETURNING *
    `)

    return this.firstRowFromResult<ImportRunFileRow>(result)
  }

  private async claimNextFileSqlite(input: ClaimInput): Promise<ImportRunFileRow | undefined> {
    const candidateRows = await this.db
      .select({ id: this.files.id })
      .from(this.files)
      .innerJoin(this.runs, eq(this.runs.id, this.files.runId))
      .where(
        and(
          eq(this.files.projectId, input.projectId),
          inArray(this.files.status, ['queued', 'retry_wait']),
          or(isNull(this.files.notBefore), lte(this.files.notBefore, input.now)),
          or(isNull(this.files.leaseUntil), lte(this.files.leaseUntil, input.now)),
          isNull(this.runs.cancelRequestedAt),
          sql`(SELECT COUNT(*) FROM import_run_files active WHERE active.project_id = ${input.projectId} AND active.status IN ('validating','running')) = 0`,
          sql`(${this.files.role} = 'records' OR NOT EXISTS (
            SELECT 1 FROM import_run_files pending_records
            WHERE pending_records.run_id = ${this.files.runId}
              AND pending_records.role = 'records'
              AND pending_records.status NOT IN (${this.terminalStatusesSql()})
          ))`
        )
      )
      .orderBy(asc(this.files.createdAt))
      .limit(1)

    const candidateId = candidateRows[0]?.id
    if (!candidateId) {
      return undefined
    }

    const result = await this.db
      .update(this.files)
      .set({
        status: 'validating',
        stage: 'validate',
        leaseOwner: input.workerId,
        leaseGeneration: sql`${this.files.leaseGeneration} + 1`,
        leaseUntil: input.leaseUntil,
        heartbeatAt: input.now,
        attemptCount: sql`${this.files.attemptCount} + 1`,
        startedAt: sql`COALESCE(${this.files.startedAt}, ${input.now})`,
        updatedAt: input.now
      })
      .where(and(eq(this.files.id, candidateId), inArray(this.files.status, ['queued', 'retry_wait'])))
      .returning()

    return result[0]
  }

  async requestCancel(runId: string, projectId: string): Promise<void> {
    const now = new Date().toISOString()
    await this.db
      .update(this.runs)
      .set({ cancelRequestedAt: now, updatedAt: now })
      .where(and(eq(this.runs.id, runId), eq(this.runs.projectId, projectId)))

    await this.db
      .update(this.files)
      .set({ cancelRequestedAt: now, status: 'canceled', finishedAt: now, updatedAt: now })
      .where(
        and(
          eq(this.files.runId, runId),
          inArray(this.files.status, [
            'awaiting_upload',
            'uploading',
            'uploaded',
            'queued',
            'blocked',
            'retry_wait'
          ])
        )
      )
  }

  // ---------- events & samples ----------

  async addEvent(event: Omit<InsertImportRunEventRow, 'id' | 'createdAt'>): Promise<void> {
    const row: InsertImportRunEventRow = { ...event, id: uuidv7(), createdAt: new Date().toISOString() }
    await this.db.insert(this.events).values(row)
  }

  async listEvents(runId: string, projectId: string, limit = 100): Promise<ImportRunEventRow[]> {
    return this.db
      .select()
      .from(this.events)
      .where(and(eq(this.events.runId, runId), eq(this.events.projectId, projectId)))
      .orderBy(desc(this.events.createdAt))
      .limit(limit)
  }

  async addErrorSample(sample: Omit<InsertImportErrorSampleRow, 'id' | 'createdAt'>): Promise<void> {
    const existing = await this.db
      .select({ count: sql<number>`count(*)` })
      .from(this.errorSamples)
      .where(eq(this.errorSamples.fileId, sample.fileId))

    if (Number(existing[0]?.count ?? 0) >= MAX_ERROR_SAMPLES_PER_FILE) {
      return
    }

    const row: InsertImportErrorSampleRow = { ...sample, id: uuidv7(), createdAt: new Date().toISOString() }
    await this.db.insert(this.errorSamples).values(row)
  }

  async listErrorSamples(fileId: string, projectId: string, limit = 50): Promise<ImportErrorSampleRow[]> {
    return this.db
      .select()
      .from(this.errorSamples)
      .where(and(eq(this.errorSamples.fileId, fileId), eq(this.errorSamples.projectId, projectId)))
      .orderBy(desc(this.errorSamples.createdAt))
      .limit(limit)
  }

  // ---------- aggregation ----------

  async refreshRunAggregates(runId: string): Promise<ImportRunRow | undefined> {
    const run = await this.getRun(runId)
    if (!run) {
      return undefined
    }

    const files = await this.listFiles(runId)
    const sum = (pick: (f: ImportRunFileRow) => number): number =>
      files.reduce((acc, f) => acc + (pick(f) ?? 0), 0)

    await this.updateRun(runId, {
      totalFiles: files.length,
      totalBytes: sum((f) => f.declaredSizeBytes ?? 0),
      uploadedBytes: sum((f) => f.objectSizeBytes ?? 0),
      parsedUnits: sum((f) => f.parsedUnits),
      recordsCommitted: sum((f) => f.recordsCommitted),
      relationshipsCommitted: sum((f) => f.relationshipsCommitted),
      skippedUnits: sum((f) => f.skippedUnits),
      failedFiles: files.filter((f) => f.status === 'failed').length
    })

    return this.getRun(runId)
  }

  async listClaimableProjects(limit = 100): Promise<string[]> {
    const rows = await this.db
      .selectDistinct({ projectId: this.files.projectId })
      .from(this.files)
      .innerJoin(this.runs, eq(this.runs.id, this.files.runId))
      .where(and(inArray(this.files.status, ['queued', 'retry_wait']), isNull(this.runs.cancelRequestedAt)))
      .limit(limit)
    return rows.map((r) => r.projectId)
  }

  /** Terminal, finalized runs whose retention window has elapsed. */
  async findRunsPastRetention(cutoffIso: string): Promise<ImportRunRow[]> {
    return this.db
      .select()
      .from(this.runs)
      .where(
        and(
          inArray(this.runs.status, ['completed', 'completed_with_errors', 'failed', 'canceled']),
          isNotNull(this.runs.finalizedAt),
          lte(this.runs.finalizedAt, cutoffIso)
        )
      )
      .limit(200)
  }

  async clearSourceForFile(fileId: string): Promise<boolean> {
    const rows = await this.db
      .update(this.files)
      .set({ storageKey: null, storageUploadId: null, updatedAt: new Date().toISOString() })
      .where(and(eq(this.files.id, fileId), isNotNull(this.files.storageKey)))
      .returning()
    return rows.length > 0
  }

  async listFilesWithSource(runId: string): Promise<ImportRunFileRow[]> {
    return this.db
      .select()
      .from(this.files)
      .where(and(eq(this.files.runId, runId), isNotNull(this.files.storageKey)))
  }

  private extractRowCount(result: unknown): number {
    if (result && typeof result === 'object') {
      const maybe = result as { rowCount?: unknown; changes?: unknown; rows?: unknown[] }
      if (typeof maybe.rowCount === 'number') {
        return maybe.rowCount
      }
      if (typeof maybe.changes === 'number') {
        return maybe.changes
      }
      if (Array.isArray(maybe.rows)) {
        return maybe.rows.length
      }
    }
    return 0
  }

  private firstRowFromResult<T>(result: unknown): T | undefined {
    if (result && typeof result === 'object') {
      const maybe = result as { rows?: unknown[] }
      if (Array.isArray(maybe.rows)) {
        return maybe.rows[0] as T | undefined
      }
      if (Array.isArray(result)) {
        return (result as unknown[])[0] as T | undefined
      }
    }
    return undefined
  }
}
