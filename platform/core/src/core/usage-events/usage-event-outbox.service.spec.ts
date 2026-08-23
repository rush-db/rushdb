import { drizzle } from 'drizzle-orm/better-sqlite3'

import { UsageEventOutboxService } from '@/core/usage-events/usage-event-outbox.service'
import { OUTBOX_MAX_ATTEMPTS } from '@/core/usage-events/usage-events.constants'
import { UsageEventV3 } from '@/core/usage-events/usage-events.interface'
import { buildIdempotencyKey } from '@/core/usage-events/write-measurements'
import { SqlService } from '@/database/sql/sql.service'

import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

// eslint-disable-next-line @typescript-eslint/no-require-imports
const Database = require('better-sqlite3')

const MIGRATION_FILE = resolve(
  __dirname,
  '../../database/sql/migrations/sqlite/0009_fixed_living_lightning.sql'
)

function makeEvent(overrides: Partial<UsageEventV3> = {}): UsageEventV3 {
  return {
    schemaVersion: 3,
    eventId: overrides.eventId ?? 'event-1',
    idempotencyKey: overrides.idempotencyKey ?? buildIdempotencyKey('ws1', 'p1', 'req1', 'advanced_query'),
    workspaceId: 'ws1',
    projectId: 'p1',
    occurredAt: new Date('2026-08-20T00:00:00Z').toISOString(),
    operationClass: 'advanced_query',
    routeOrTool: '/records/search',
    source: 'rest',
    outcome: 'succeeded',
    costDimensions: {
      targetScopeRecords: 2_000_000,
      traversalDepth: 2,
      aggregation: true
    },
    classificationVersion: 'test',
    deployment: 'managed',
    ...overrides
  }
}

describe('UsageEventOutboxService (sqlite integration)', () => {
  let outbox: UsageEventOutboxService
  let sqlService: SqlService

  beforeAll(() => {
    const client = new Database(':memory:')
    // Apply the generated outbox migration (split on drizzle's breakpoint).
    const migration = readFileSync(MIGRATION_FILE, 'utf8')
    for (const statement of migration.split('--> statement-breakpoint')) {
      client.exec(statement.trim())
    }
    const db = drizzle(client)
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    sqlService = new SqlService(db, require('@/database/sql/schema/sqlite.schema'), 'sqlite')
    outbox = new UsageEventOutboxService(sqlService)
  })

  it('enqueues an event and claims it when due', async () => {
    await outbox.enqueue(makeEvent())

    const due = await outbox.claimDueBatch(new Date())
    expect(due).toHaveLength(1)
    expect(JSON.parse(due[0].payload).eventId).toBe('event-1')
    expect(due[0].status).toBe('pending')
  })

  it('deduplicates events by idempotency key', async () => {
    const key = buildIdempotencyKey('ws-dup', 'p1', 'req-dup', 'write')

    await outbox.enqueue(makeEvent({ eventId: 'dup-a', idempotencyKey: key }))
    // Transport retry of the same logical operation under a new event id — no-op.
    await outbox.enqueue(makeEvent({ eventId: 'dup-b', idempotencyKey: key }))

    const due = await outbox.claimDueBatch(new Date())
    const matching = due.filter((row) => row.idempotencyKey === key)
    expect(matching).toHaveLength(1)
    expect(matching[0].id).toBe('dup-a')
  })

  it('does not claim events whose next attempt is in the future', async () => {
    await outbox.enqueue(
      makeEvent({
        eventId: 'future-event',
        idempotencyKey: buildIdempotencyKey('ws-future', 'p1', 'req-future', 'write')
      })
    )
    const all = await outbox.claimDueBatch(new Date())
    const row = all.find((r) => r.id === 'future-event')
    expect(row).toBeDefined()
    await outbox.markFailed([{ row: row!, error: 'boom' }], new Date())

    const dueNow = await outbox.claimDueBatch(new Date())
    expect(dueNow.map((r) => r.id)).not.toContain('future-event')

    const dueLater = await outbox.claimDueBatch(new Date('2027-01-01T00:00:00Z'))
    expect(dueLater.map((r) => r.id)).toContain('future-event')
  })

  it('marks delivered rows as acknowledged', async () => {
    await outbox.enqueue(
      makeEvent({ eventId: 'ok-event', idempotencyKey: buildIdempotencyKey('ws-ok', 'p1', 'r', 'write') })
    )
    const all = await outbox.claimDueBatch(new Date())
    const row = all.find((r) => r.id === 'ok-event')
    expect(row).toBeDefined()
    await outbox.markDelivered([row!.id])

    const due = await outbox.claimDueBatch(new Date())
    expect(due.map((r) => r.id)).not.toContain('ok-event')
  })

  it('dead-letters rows after the maximum number of attempts but keeps them replayable', async () => {
    const key = buildIdempotencyKey('ws-dead', 'p1', 'req-dead', 'write')
    await outbox.enqueue(makeEvent({ eventId: 'dead-event', idempotencyKey: key }))

    // Backoff is capped well below one day, so claiming one day further out
    // per attempt always finds the row again until it dead-letters.
    for (let attempt = 1; attempt <= OUTBOX_MAX_ATTEMPTS; attempt++) {
      const claimedAt = new Date(Date.now() + attempt * 24 * 60 * 60 * 1000)
      const due = await outbox.claimDueBatch(claimedAt)
      const row = due.find((r) => r.id === 'dead-event')
      expect(row).toBeDefined()

      await outbox.markFailed([{ row: row!, error: 'delivery endpoint down' }], claimedAt)
    }

    const muchLater = await outbox.claimDueBatch(new Date('2027-06-01T00:00:00Z'))
    const deadRow = muchLater.find((r) => r.id === 'dead-event')
    expect(deadRow).toBeUndefined() // no longer claimable

    const rows: Array<{ id: string; status: string; attemptCount: number; lastError: string | null }> =
      await sqlService.db.select().from(sqlService.tables.usageEventOutbox)
    const deadRowPersisted = rows.find((r) => r.id === 'dead-event')

    expect(deadRowPersisted).toBeDefined()
    expect(deadRowPersisted!.status).toBe('dead')
    expect(Number(deadRowPersisted!.attemptCount)).toBe(OUTBOX_MAX_ATTEMPTS)
    expect(deadRowPersisted!.lastError).toContain('delivery endpoint down')
  })
})
