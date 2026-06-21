import { Injectable } from '@nestjs/common'
import { and, desc, eq, or } from 'drizzle-orm'

import { SqlService } from '@/database/sql/sql.service'

import type {
  ConnectorEventRow,
  ConnectorOffsetRow,
  ConnectorRow,
  ConnectorSecretRow,
  InsertConnectorEventRow,
  InsertConnectorLeaseRow,
  InsertConnectorOffsetRow,
  InsertConnectorRow,
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

  async findRunnableWithoutLease(limit = 10): Promise<ConnectorRow[]> {
    const now = new Date().toISOString()
    await this.deleteExpiredLeases(now)
    const leased = await this.db
      .select({ connectorId: this.connectorLeases.connectorId, leaseUntil: this.connectorLeases.leaseUntil })
      .from(this.connectorLeases)
    const leasedIds = leased.filter((row) => row.leaseUntil > now).map((row) => row.connectorId)

    // Drizzle has no cross-dialect NOT IN helper in the current local style.
    const rows = await this.db
      .select()
      .from(this.connectors)
      .where(eq(this.connectors.status, 'running'))
      .limit(limit)
    return rows.filter((row) => !leasedIds.includes(row.id))
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

  async deleteLease(connectorId: string): Promise<void> {
    await this.db.delete(this.connectorLeases).where(eq(this.connectorLeases.connectorId, connectorId))
  }

  async deleteExpiredLeases(nowIso: string): Promise<void> {
    const rows = await this.db.select().from(this.connectorLeases)
    const expired = rows.filter((row) => row.leaseUntil <= nowIso)

    for (const row of expired) {
      await this.deleteLease(row.connectorId)
    }
  }
}
