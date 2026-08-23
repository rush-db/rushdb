import { Injectable, Logger } from '@nestjs/common'
import { and, asc, eq, inArray, lte, sql } from 'drizzle-orm'

import { computeBackoffMs } from '@/core/usage-events/outbox-backoff'
import {
  OUTBOX_BATCH_SIZE,
  OUTBOX_MAX_ATTEMPTS,
  OutboxStatus
} from '@/core/usage-events/usage-events.constants'
import { UsageEventV3 } from '@/core/usage-events/usage-events.interface'
import { SqlService } from '@/database/sql/sql.service'

interface OutboxRow {
  id: string
  idempotencyKey: string
  workspaceId: string
  projectId: string | null
  operationClass: string
  payload: string
  status: OutboxStatus | string
  attemptCount: number
  nextAttemptAt: string
  lastError: string | null
  createdAt: string
  acknowledgedAt: string | null
}

/**
 * Durable outbox for usage events.
 *
 * Events are persisted to core SQL before any delivery attempt. Delivery is
 * retried with exponential backoff + jitter until the delivery endpoint
 * acknowledges them; after OUTBOX_MAX_ATTEMPTS a row is dead-lettered (kept,
 * never deleted) so operations can replay it.
 *
 * Duplicate delivery of the same logical operation is impossible by the
 * unique idempotency key — re-inserting an already-known event is a no-op.
 */
@Injectable()
export class UsageEventOutboxService {
  private readonly logger = new Logger(UsageEventOutboxService.name)

  constructor(private readonly sqlService: SqlService) {}

  private get table() {
    return this.sqlService.tables.usageEventOutbox
  }

  /**
   * Persists an event for delivery. Inserting an event whose idempotency key
   * already exists is a no-op — transport retries and replays never create
   * duplicate usage.
   */
  async enqueue(event: UsageEventV3): Promise<void> {
    const now = new Date().toISOString()

    await this.sqlService.db
      .insert(this.table)
      .values({
        id: event.eventId,
        idempotencyKey: event.idempotencyKey,
        workspaceId: event.workspaceId,
        projectId: event.projectId ?? null,
        operationClass: event.operationClass,
        payload: JSON.stringify(event),
        status: 'pending',
        attemptCount: 0,
        nextAttemptAt: now,
        createdAt: now
      })
      .onConflictDoNothing()
  }

  /** Returns pending rows whose next attempt is due, oldest first. */
  async claimDueBatch(now: Date = new Date(), limit: number = OUTBOX_BATCH_SIZE): Promise<OutboxRow[]> {
    const rows: OutboxRow[] = await this.sqlService.db
      .select()
      .from(this.table)
      .where(and(eq(this.table.status, 'pending'), lte(this.table.nextAttemptAt, now.toISOString())))
      .orderBy(asc(this.table.createdAt))
      .limit(limit)

    return rows
  }

  /** Marks rows as delivered by their event ids. */
  async markDelivered(eventIds: string[], acknowledgedAt: Date = new Date()): Promise<void> {
    if (eventIds.length === 0) {
      return
    }

    await this.sqlService.db
      .update(this.table)
      .set({ status: 'delivered', acknowledgedAt: acknowledgedAt.toISOString(), lastError: null })
      .where(inArray(this.table.id, eventIds))
  }

  /**
   * Records a failed delivery round for each row: bumps the attempt counter
   * and schedules the next attempt with exponential backoff + jitter. Rows
   * that exceed OUTBOX_MAX_ATTEMPTS are dead-lettered (status 'dead'), never
   * deleted — they stay replayable.
   */
  async markFailed(
    failures: Array<{ row: OutboxRow; error: string }>,
    now: Date = new Date()
  ): Promise<void> {
    if (failures.length === 0) {
      return
    }

    for (const { row, error } of failures) {
      const attemptCount = Number(row.attemptCount ?? 0) + 1
      const dead = attemptCount >= OUTBOX_MAX_ATTEMPTS

      if (dead) {
        await this.sqlService.db
          .update(this.table)
          .set({
            status: 'dead',
            attemptCount,
            lastError: error.slice(0, 2000)
          })
          .where(eq(this.table.id, row.id))

        this.logger.warn(`Usage event ${row.id} dead-lettered after ${attemptCount} attempts`)
        continue
      }

      const delayMs = computeBackoffMs(attemptCount)
      const nextAttemptAt = new Date(now.getTime() + delayMs).toISOString()

      await this.sqlService.db
        .update(this.table)
        .set({
          status: 'pending',
          attemptCount,
          nextAttemptAt,
          lastError: error.slice(0, 2000)
        })
        .where(eq(this.table.id, row.id))
    }
  }

  /** Count of pending events (operational visibility). */
  async countPending(): Promise<number> {
    const rows = await this.sqlService.db
      .select({ count: sql<number>`count(*)` })
      .from(this.table)
      .where(eq(this.table.status, 'pending'))

    return Number(rows[0]?.count ?? 0)
  }
}
