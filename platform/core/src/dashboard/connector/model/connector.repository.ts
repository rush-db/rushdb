import { Injectable } from '@nestjs/common'
import { and, asc, desc, eq, gt, like, lte, ne, notInArray, or, sql } from 'drizzle-orm'

import { SqlService } from '@/database/sql/sql.service'

import type {
  ConnectorCommandRow,
  ConnectorDefinitionRow,
  ConnectorEventRow,
  ConnectorOffsetRow,
  ConnectorRejectionRow,
  ConnectorRow,
  ConnectorRunRow,
  ConnectorSecretRow,
  InsertConnectorCommandRow,
  InsertConnectorDefinitionRow,
  InsertConnectorEventRow,
  InsertConnectorLeaseRow,
  InsertConnectorOffsetRow,
  InsertConnectorRejectionRow,
  InsertConnectorRow,
  InsertConnectorRunRow,
  InsertConnectorSecretRow
} from '@/database/sql/schema/types'

@Injectable()
export class ConnectorRepository {
  constructor(private readonly sql: SqlService) {}

  private get db() {
    return this.sql.db
  }
  private get connectors() {
    return this.sql.tables.connectors
  }
  private get connectorSecrets() {
    return this.sql.tables.connectorSecrets
  }
  private get connectorOffsets() {
    return this.sql.tables.connectorOffsets
  }
  private get connectorEvents() {
    return this.sql.tables.connectorEvents
  }
  private get connectorLeases() {
    return this.sql.tables.connectorLeases
  }
  private get connectorCommands() {
    return this.sql.tables.connectorCommands
  }
  private get connectorRuns() {
    return this.sql.tables.connectorRuns
  }
  private get connectorRejections() {
    return this.sql.tables.connectorRejections
  }
  private get connectorDefinitions() {
    return this.sql.tables.connectorDefinitions
  }

  async create(data: InsertConnectorRow): Promise<ConnectorRow> {
    await this.db.insert(this.connectors).values(data)
    return this.findById(data.id)
  }

  async findById(id: string): Promise<ConnectorRow | undefined> {
    const rows = await this.db.select().from(this.connectors).where(eq(this.connectors.id, id))
    return rows[0]
  }

  async findByProjectId(projectId: string): Promise<ConnectorRow[]> {
    return this.db
      .select()
      .from(this.connectors)
      .where(
        and(
          eq(this.connectors.projectId, projectId),
          or(
            eq(this.connectors.status, 'paused'),
            eq(this.connectors.status, 'running'),
            eq(this.connectors.status, 'error'),
            eq(this.connectors.status, 'testing')
          )
        )
      )
  }

  async update(id: string, data: Partial<Omit<InsertConnectorRow, 'id'>>): Promise<ConnectorRow | undefined> {
    await this.db.update(this.connectors).set(data).where(eq(this.connectors.id, id))
    return this.findById(id)
  }

  async delete(id: string): Promise<boolean> {
    const row = await this.findById(id)
    if (!row) {
      return false
    }
    await this.db.delete(this.connectors).where(eq(this.connectors.id, id))
    return true
  }

  async upsertSecret(data: InsertConnectorSecretRow): Promise<void> {
    await this.db
      .insert(this.connectorSecrets)
      .values(data)
      .onConflictDoUpdate({
        target: this.connectorSecrets.connectorId,
        set: {
          provider: data.provider,
          secretRef: data.secretRef,
          ciphertext: data.ciphertext,
          updatedAt: data.updatedAt
        }
      })
  }

  async findSecret(connectorId: string): Promise<ConnectorSecretRow | undefined> {
    const rows = await this.db
      .select()
      .from(this.connectorSecrets)
      .where(eq(this.connectorSecrets.connectorId, connectorId))
    return rows[0]
  }

  async upsertOffset(data: InsertConnectorOffsetRow): Promise<void> {
    await this.db
      .insert(this.connectorOffsets)
      .values(data)
      .onConflictDoUpdate({
        target: [this.connectorOffsets.connectorId, this.connectorOffsets.partition],
        set: { position: data.position, updatedAt: data.updatedAt }
      })
  }

  async findOffsets(connectorId: string): Promise<ConnectorOffsetRow[]> {
    return this.db
      .select()
      .from(this.connectorOffsets)
      .where(eq(this.connectorOffsets.connectorId, connectorId))
  }

  /**
   * Delete a connector's offsets whose partition starts with `prefix`
   * (e.g. `snapshot:` for resnapshot, `cs:` for change-stream tokens).
   */
  async deleteOffsetsByPrefix(connectorId: string, prefix: string): Promise<void> {
    await this.db
      .delete(this.connectorOffsets)
      .where(
        and(
          eq(this.connectorOffsets.connectorId, connectorId),
          like(this.connectorOffsets.partition, `${prefix}%`)
        )
      )
  }

  async addEvent(data: InsertConnectorEventRow): Promise<ConnectorEventRow> {
    await this.db.insert(this.connectorEvents).values(data)
    const rows = await this.db.select().from(this.connectorEvents).where(eq(this.connectorEvents.id, data.id))
    return rows[0]
  }

  async findEvents(connectorId: string, limit = 50): Promise<ConnectorEventRow[]> {
    return this.db
      .select()
      .from(this.connectorEvents)
      .where(eq(this.connectorEvents.connectorId, connectorId))
      .orderBy(desc(this.connectorEvents.createdAt))
      .limit(limit)
  }

  /**
   * Atomically claim the next unleased `running` connector for `workerId`.
   *
   * Single SQL statement: `INSERT INTO connector_leases ... SELECT ... WHERE
   * status='running' AND id NOT IN (active leases) LIMIT 1 ON CONFLICT DO
   * NOTHING`. The DB does the lease check + insert atomically, so concurrent
   * workers can never double-claim, and there is no fetch-then-filter over the
   * whole connector table (which did not scale to many running connectors).
   *
   * Returns the claimed connector id, or `undefined` when nothing is available.
   */
  async claimNextRunnable(
    workerId: string,
    nowIso: string,
    leaseUntilIso: string
  ): Promise<string | undefined> {
    const activeLease = this.db
      .select({ connectorId: this.connectorLeases.connectorId })
      .from(this.connectorLeases)
      .where(gt(this.connectorLeases.leaseUntil, nowIso))

    // insert-select without returning() does not populate `rows`; returning()
    // yields the leased connector id on success. ON CONFLICT DO UPDATE steals
    // an EXPIRED lease (same connector_id PK) — a still-active lease is left
    // untouched (DO NOTHING), so concurrent workers can never double-claim.
    const selected = await this.db
      .insert(this.connectorLeases)
      .select((qb) =>
        qb
          .select({
            connectorId: this.connectors.id,
            workerId: sql`${workerId}`.as('worker_id'),
            leaseUntil: sql`${leaseUntilIso}`.as('lease_until'),
            heartbeatAt: sql`${nowIso}`.as('heartbeat_at'),
            createdAt: sql`${nowIso}`.as('created_at'),
            updatedAt: sql`${nowIso}`.as('updated_at')
          })
          .from(this.connectors)
          .where(and(eq(this.connectors.status, 'running'), notInArray(this.connectors.id, activeLease)))
          .limit(1)
      )
      .onConflictDoUpdate({
        target: this.connectorLeases.connectorId,
        set: {
          workerId,
          leaseUntil: leaseUntilIso,
          heartbeatAt: nowIso,
          createdAt: nowIso,
          updatedAt: nowIso
        },
        where: lte(this.connectorLeases.leaseUntil, nowIso)
      })
      .returning({ connectorId: this.connectorLeases.connectorId })

    return selected[0]?.connectorId
  }

  async findLease(connectorId: string) {
    const rows = await this.db
      .select()
      .from(this.connectorLeases)
      .where(eq(this.connectorLeases.connectorId, connectorId))
    return rows[0]
  }

  async upsertLease(data: InsertConnectorLeaseRow): Promise<void> {
    await this.db
      .insert(this.connectorLeases)
      .values(data)
      .onConflictDoUpdate({
        target: this.connectorLeases.connectorId,
        set: {
          workerId: data.workerId,
          leaseUntil: data.leaseUntil,
          heartbeatAt: data.heartbeatAt,
          updatedAt: data.updatedAt
        }
      })
  }

  /**
   * Renew a connector's lease ONLY if it is still owned by `workerId` and not
   * expired. Returns `true` when renewed. This is the cheap single-statement
   * heartbeat: the conditional update both checks ownership and extends the
   * TTL, so a stale worker cannot extend a lease it no longer owns.
   */
  async renewLeaseIfOwned(
    connectorId: string,
    workerId: string,
    leaseUntilIso: string,
    heartbeatAtIso: string
  ): Promise<boolean> {
    const result = await this.db
      .update(this.connectorLeases)
      .set({
        leaseUntil: leaseUntilIso,
        heartbeatAt: heartbeatAtIso,
        updatedAt: heartbeatAtIso
      })
      .where(
        and(
          eq(this.connectorLeases.connectorId, connectorId),
          eq(this.connectorLeases.workerId, workerId),
          gt(this.connectorLeases.leaseUntil, heartbeatAtIso)
        )
      )
      .returning({ connectorId: this.connectorLeases.connectorId })
    return result.length > 0
  }

  async deleteLease(connectorId: string): Promise<void> {
    await this.db.delete(this.connectorLeases).where(eq(this.connectorLeases.connectorId, connectorId))
  }

  async deleteExpiredLeases(nowIso: string): Promise<void> {
    await this.db.delete(this.connectorLeases).where(lte(this.connectorLeases.leaseUntil, nowIso))
  }

  async createCommand(data: InsertConnectorCommandRow): Promise<ConnectorCommandRow> {
    await this.db.insert(this.connectorCommands).values(data)
    return this.findCommandById(data.id)
  }

  async findCommandById(id: string): Promise<ConnectorCommandRow | undefined> {
    const rows = await this.db.select().from(this.connectorCommands).where(eq(this.connectorCommands.id, id))
    return rows[0]
  }

  async findPendingCommand(limit = 1): Promise<ConnectorCommandRow[]> {
    return this.db
      .select()
      .from(this.connectorCommands)
      .where(eq(this.connectorCommands.status, 'pending'))
      .limit(limit)
  }

  async findCommands(connectorId: string, limit = 20): Promise<ConnectorCommandRow[]> {
    return this.db
      .select()
      .from(this.connectorCommands)
      .where(eq(this.connectorCommands.connectorId, connectorId))
      .orderBy(desc(this.connectorCommands.createdAt))
      .limit(limit)
  }

  async claimCommand(id: string, workerId: string): Promise<ConnectorCommandRow | undefined> {
    const rows = await this.db.select().from(this.connectorCommands).where(eq(this.connectorCommands.id, id))
    const row = rows[0]
    if (!row || row.status !== 'pending') {
      return undefined
    }
    await this.db
      .update(this.connectorCommands)
      .set({
        status: 'claimed',
        claimedBy: workerId,
        claimedAt: new Date().toISOString()
      })
      .where(eq(this.connectorCommands.id, id))
    return this.findCommandById(id)
  }

  async completeCommand(
    id: string,
    data: { status: 'completed' | 'failed'; result?: string; errorMessage?: string }
  ): Promise<ConnectorCommandRow | undefined> {
    await this.db
      .update(this.connectorCommands)
      .set({
        status: data.status,
        result: data.result,
        errorMessage: data.errorMessage,
        completedAt: new Date().toISOString()
      })
      .where(eq(this.connectorCommands.id, id))
    return this.findCommandById(id)
  }

  // --- durable runs ---------------------------------------------------------

  async createRun(data: InsertConnectorRunRow): Promise<ConnectorRunRow> {
    await this.db.insert(this.connectorRuns).values(data)
    return this.findRunById(data.id)
  }

  async findRunById(id: string): Promise<ConnectorRunRow | undefined> {
    const rows = await this.db.select().from(this.connectorRuns).where(eq(this.connectorRuns.id, id))
    return rows[0]
  }

  async findRuns(connectorId: string, limit = 20): Promise<ConnectorRunRow[]> {
    return this.db
      .select()
      .from(this.connectorRuns)
      .where(eq(this.connectorRuns.connectorId, connectorId))
      .orderBy(desc(this.connectorRuns.startedAt))
      .limit(limit)
  }

  async updateRun(
    id: string,
    data: Partial<{
      status: string
      phase: string
      recordsRead: number
      recordsWritten: number
      recordsRejected: number
      errorMessage: string | null
      completedAt: string | null
      heartbeatAt: string
    }>
  ): Promise<ConnectorRunRow | undefined> {
    await this.db.update(this.connectorRuns).set(data).where(eq(this.connectorRuns.id, id))
    return this.findRunById(id)
  }

  async findActiveRuns(connectorId: string): Promise<ConnectorRunRow[]> {
    return this.db
      .select()
      .from(this.connectorRuns)
      .where(and(eq(this.connectorRuns.connectorId, connectorId), eq(this.connectorRuns.status, 'running')))
  }

  // --- rejection ledger (metadata only; never payloads) ----------------------

  /**
   * Upsert a rejection by (connector, code, sourceIdHash): increments the
   * occurrence counter and refreshes lastSeenAt instead of appending a row per
   * event. Only metadata (code, hashed id, redacted message) is stored.
   */
  async upsertRejection(
    data: Omit<InsertConnectorRejectionRow, 'occurrenceCount' | 'firstSeenAt' | 'lastSeenAt' | 'createdAt'>
  ): Promise<ConnectorRejectionRow> {
    const now = new Date().toISOString()
    await this.db
      .insert(this.connectorRejections)
      .values({
        ...data,
        occurrenceCount: 1,
        firstSeenAt: now,
        lastSeenAt: now,
        resolved: 0,
        createdAt: now
      })
      .onConflictDoUpdate({
        target: [
          this.connectorRejections.connectorId,
          this.connectorRejections.code,
          this.connectorRejections.sourceIdHash
        ],
        set: {
          lastSeenAt: now,
          occurrenceCount: sql`${this.connectorRejections.occurrenceCount} + 1`
        }
      })
    return this.findRejectionByKey(data.connectorId, data.code, data.sourceIdHash)
  }

  async findRejectionByKey(
    connectorId: string,
    code: string,
    sourceIdHash: string | null | undefined
  ): Promise<ConnectorRejectionRow | undefined> {
    const rows = await this.db
      .select()
      .from(this.connectorRejections)
      .where(
        and(
          eq(this.connectorRejections.connectorId, connectorId),
          eq(this.connectorRejections.code, code),
          sourceIdHash === null || sourceIdHash === undefined ?
            eq(this.connectorRejections.sourceIdHash, null)
          : eq(this.connectorRejections.sourceIdHash, sourceIdHash)
        )
      )
    return rows[0]
  }

  async findRejections(connectorId: string, limit = 50): Promise<ConnectorRejectionRow[]> {
    return this.db
      .select()
      .from(this.connectorRejections)
      .where(eq(this.connectorRejections.connectorId, connectorId))
      .orderBy(desc(this.connectorRejections.lastSeenAt))
      .limit(limit)
  }

  async resolveRejections(connectorId: string): Promise<void> {
    await this.db
      .update(this.connectorRejections)
      .set({ resolved: 1 })
      .where(eq(this.connectorRejections.connectorId, connectorId))
  }

  // --- connector definitions (runtime-registered catalog) -------------------

  async upsertConnectorDefinition(data: InsertConnectorDefinitionRow): Promise<ConnectorDefinitionRow> {
    await this.db
      .insert(this.connectorDefinitions)
      .values(data)
      .onConflictDoUpdate({
        target: this.connectorDefinitions.id,
        set: {
          descriptor: data.descriptor,
          version: data.version,
          registeredBy: data.registeredBy,
          updatedAt: data.updatedAt
        }
      })
    return this.findConnectorDefinition(data.id)
  }

  async findConnectorDefinition(id: string): Promise<ConnectorDefinitionRow | undefined> {
    const rows = await this.db
      .select()
      .from(this.connectorDefinitions)
      .where(eq(this.connectorDefinitions.id, id))
    return rows[0]
  }

  async listConnectorDefinitions(): Promise<ConnectorDefinitionRow[]> {
    return this.db.select().from(this.connectorDefinitions).orderBy(asc(this.connectorDefinitions.id))
  }

  async deleteConnectorDefinition(id: string): Promise<void> {
    await this.db.delete(this.connectorDefinitions).where(eq(this.connectorDefinitions.id, id))
  }
}
