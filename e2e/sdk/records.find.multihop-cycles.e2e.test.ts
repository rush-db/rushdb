/**
 * E2E tests for variable-length traversal (`$relation.hops`) and cycle
 * detection (`$cycle`) in `records.find()` where clauses.
 *
 * Data model (typed relationships created via relationships.createMany):
 *
 *   Reporting chain (MHEmployee, REPORTS_TO, directed "up"):
 *     E1 -REPORTS_TO-> E2 -REPORTS_TO-> E3 -REPORTS_TO-> E4
 *
 *   Transfer ring + innocent linear chain (MHAccount, TRANSFERRED_TO):
 *     A -> B -> C -> A          (3-hop directed ring)
 *     X -> Y -> Z               (linear, no cycle)
 *
 *   Isolated pair (MHIso): two records sharing property values but with NO
 *   relationships — guards against untyped multihop leaking through the
 *   internal property meta-node edges.
 *
 * Each record carries a `tenantId` so tests are fully isolated from each
 * other and from any leftover data in the database.
 *
 * Prerequisites
 * ─────────────
 * RUSHDB_API_KEY and RUSHDB_API_URL must be set in packages/javascript-sdk/.env
 *
 * If RUSHDB_API_KEY is absent the whole suite is skipped gracefully.
 */

import RushDB from '../../packages/javascript-sdk/src/index.node'

jest.setTimeout(120_000)

const sleep = (ms: number) => new Promise((res) => setTimeout(res, ms))

const apiKey = process.env.RUSHDB_API_KEY
const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

if (!apiKey) {
  describe('records.find() – multihop & cycles (e2e)', () => {
    it('skips because RUSHDB_API_KEY is not set', () => expect(true).toBe(true))
  })
} else {
  const db = new RushDB(apiKey, { url: apiUrl })

  // Unique tenant so parallel runs never collide
  const tenantId = `multihop-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

  // ── Labels ────────────────────────────────────────────────────────────────
  const L_EMPLOYEE = 'MHEmployee'
  const L_ACCOUNT = 'MHAccount'
  const L_ISO = 'MHIso'

  const names = (res: { data: unknown }) => (res.data as any[]).map((r) => r.data.name).sort()

  // ── Setup ─────────────────────────────────────────────────────────────────
  beforeAll(async () => {
    await db.records.delete({ where: { tenantId } }).catch(() => {})

    // Reporting chain: each employee points at their manager by name.
    await db.records.createMany({
      label: L_EMPLOYEE,
      data: [
        { name: 'E1', managerName: 'E2', tenantId },
        { name: 'E2', managerName: 'E3', tenantId },
        { name: 'E3', managerName: 'E4', tenantId },
        { name: 'E4', tenantId }
      ],
      options: { returnResult: false }
    })
    await db.relationships.createMany({
      source: { label: L_EMPLOYEE, key: 'managerName', where: { tenantId } },
      target: { label: L_EMPLOYEE, key: 'name', where: { tenantId } },
      type: 'REPORTS_TO',
      direction: 'out'
    })

    // Transfer ring A -> B -> C -> A and linear chain X -> Y -> Z.
    await db.records.createMany({
      label: L_ACCOUNT,
      data: [
        { name: 'A', sendsTo: 'B', tenantId },
        { name: 'B', sendsTo: 'C', tenantId },
        { name: 'C', sendsTo: 'A', tenantId },
        { name: 'X', sendsTo: 'Y', tenantId },
        { name: 'Y', sendsTo: 'Z', tenantId },
        { name: 'Z', tenantId }
      ],
      options: { returnResult: false }
    })
    await db.relationships.createMany({
      source: { label: L_ACCOUNT, key: 'sendsTo', where: { tenantId } },
      target: { label: L_ACCOUNT, key: 'name', where: { tenantId } },
      type: 'TRANSFERRED_TO',
      direction: 'out' // sender -> receiver
    })

    // Disconnected records sharing a property value (meta-node leakage guard).
    await db.records.createMany({
      label: L_ISO,
      data: [
        { name: 'Iso1', sharedTag: 'common-value', tenantId },
        { name: 'Iso2', sharedTag: 'common-value', tenantId }
      ],
      options: { returnResult: false }
    })

    // relationships.createMany runs through apoc.periodic.iterate — wait until
    // both relationship sets are visible before running assertions.
    let attempts = 0
    while (attempts < 10) {
      const [employeeRels, accountRels] = await Promise.all([
        db.relationships.find({ source: { labels: [L_EMPLOYEE], where: { tenantId } }, limit: 100 }),
        db.relationships.find({ source: { labels: [L_ACCOUNT], where: { tenantId } }, limit: 100 })
      ])
      if ((employeeRels.data as any[]).length >= 3 && (accountRels.data as any[]).length >= 5) break
      attempts += 1
      await sleep(1000)
    }
  })

  // ── Teardown ──────────────────────────────────────────────────────────────
  afterAll(async () => {
    await db.records.delete({ where: { tenantId } }).catch(() => {})
  })

  describe('records.find() – multihop & cycles (e2e)', () => {
    // ══════════════════════════════════════════════════════════════════════
    // 1. Variable-length traversal — typed
    // ══════════════════════════════════════════════════════════════════════
    describe('$relation.hops (typed)', () => {
      it('reaches the chain top within max hops', async () => {
        const res = await db.records.find({
          labels: [L_EMPLOYEE],
          where: {
            tenantId,
            [L_EMPLOYEE]: {
              $relation: { type: 'REPORTS_TO', direction: 'out', hops: { max: 3 } },
              name: 'E4'
            }
          }
        })
        expect(names(res)).toEqual(['E1', 'E2', 'E3'])
      })

      it('does not reach beyond max hops', async () => {
        const res = await db.records.find({
          labels: [L_EMPLOYEE],
          where: {
            tenantId,
            [L_EMPLOYEE]: {
              $relation: { type: 'REPORTS_TO', direction: 'out', hops: { max: 2 } },
              name: 'E4'
            }
          }
        })
        expect(names(res)).toEqual(['E2', 'E3'])
      })

      it('matches an exact hop count', async () => {
        const res = await db.records.find({
          labels: [L_EMPLOYEE],
          where: {
            tenantId,
            [L_EMPLOYEE]: {
              $relation: { type: 'REPORTS_TO', direction: 'out', hops: 3 },
              name: 'E4'
            }
          }
        })
        expect(names(res)).toEqual(['E1'])
      })

      it('respects min hops', async () => {
        const res = await db.records.find({
          labels: [L_EMPLOYEE],
          where: {
            tenantId,
            [L_EMPLOYEE]: {
              $relation: { type: 'REPORTS_TO', direction: 'out', hops: { min: 2, max: 3 } },
              name: 'E4'
            }
          }
        })
        expect(names(res)).toEqual(['E1', 'E2'])
      })
    })

    // ══════════════════════════════════════════════════════════════════════
    // 2. Variable-length traversal — untyped
    // ══════════════════════════════════════════════════════════════════════
    describe('$relation.hops (untyped)', () => {
      it('traverses any relationship type when type is omitted', async () => {
        const res = await db.records.find({
          labels: [L_EMPLOYEE],
          where: {
            tenantId,
            [L_EMPLOYEE]: {
              $relation: { direction: 'out', hops: { max: 3 } },
              name: 'E4'
            }
          }
        })
        expect(names(res)).toEqual(['E1', 'E2', 'E3'])
      })

      it('does not fabricate connectivity through shared property values', async () => {
        const res = await db.records.find({
          labels: [L_ISO],
          where: {
            tenantId,
            name: 'Iso1',
            [L_ISO]: {
              $relation: { hops: { max: 4 } },
              name: 'Iso2'
            }
          }
        })
        expect(res.total).toBe(0)
      })
    })

    // ══════════════════════════════════════════════════════════════════════
    // 3. Cycle detection
    // ══════════════════════════════════════════════════════════════════════
    describe('$cycle', () => {
      it('flags exactly the ring members, with a path-deduplicated total', async () => {
        const res = await db.records.find({
          labels: [L_ACCOUNT],
          where: {
            tenantId,
            RING: {
              $cycle: true,
              $relation: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 2, max: 6 } }
            }
          }
        })
        expect(names(res)).toEqual(['A', 'B', 'C'])
        expect(res.total).toBe(3)
      })

      it('excludes ring members with $not (acyclic accounts)', async () => {
        const res = await db.records.find({
          labels: [L_ACCOUNT],
          where: {
            tenantId,
            $not: {
              RING: {
                $cycle: true,
                $relation: { type: 'TRANSFERRED_TO', direction: 'out', hops: { min: 2, max: 6 } }
              }
            }
          }
        })
        expect(names(res)).toEqual(['X', 'Y', 'Z'])
      })
    })

    // ══════════════════════════════════════════════════════════════════════
    // 4. Validation
    // ══════════════════════════════════════════════════════════════════════
    describe('validation', () => {
      it('rejects zero hops', async () => {
        await expect(
          db.records.find({
            labels: [L_EMPLOYEE],
            where: { tenantId, [L_EMPLOYEE]: { $relation: { type: 'REPORTS_TO', hops: 0 } } }
          })
        ).rejects.toThrow()
      })

      it('rejects min greater than max', async () => {
        await expect(
          db.records.find({
            labels: [L_EMPLOYEE],
            where: {
              tenantId,
              [L_EMPLOYEE]: { $relation: { type: 'REPORTS_TO', hops: { min: 5, max: 2 } } }
            }
          })
        ).rejects.toThrow()
      })

      it('rejects $cycle with $alias', async () => {
        await expect(
          db.records.find({
            labels: [L_ACCOUNT],
            where: {
              tenantId,
              RING: {
                $cycle: true,
                $alias: '$ring',
                $relation: { type: 'TRANSFERRED_TO', hops: { min: 2, max: 4 } }
              }
            }
          })
        ).rejects.toThrow()
      })

      it('rejects $cycle without hops', async () => {
        await expect(
          db.records.find({
            labels: [L_ACCOUNT],
            where: { tenantId, RING: { $cycle: true, $relation: { type: 'TRANSFERRED_TO' } } }
          })
        ).rejects.toThrow()
      })
    })
  })
}
