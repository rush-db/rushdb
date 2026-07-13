/**
 * E2E regression test for a bug where a related-label `where` traversal filter
 * combined with `skip`/`limit` returned incomplete `data` while `total` stayed
 * correct.
 *
 * Root cause (fixed in EntityQueryService.findRecords): SKIP/LIMIT/ORDER BY were
 * glued onto the root MATCH clause, which ran *before* the OPTIONAL MATCH
 * traversal to the related label and before the WITH...WHERE barrier that
 * applies the traversal predicate. So pagination sliced the raw, unfiltered
 * root scan first, and only afterwards was the traversal filter applied to
 * whatever records happened to survive that slice — silently dropping matches
 * that lived outside the pagination window.
 *
 * This test pins down an explicit, deterministic ordering (via a `seq`
 * property) so the reproduction doesn't depend on incidental id ordering:
 * parent "A" owns children seq 0-149, parent "B" owns children seq 150-299.
 * Querying children scoped to "B", ordered by seq ascending, means the first
 * `limit` window of the *unfiltered* scan is entirely "A"'s children — exactly
 * the failure mode from the bug report.
 *
 * Prerequisites
 * ─────────────
 * RUSHDB_API_KEY and RUSHDB_API_URL must be set in packages/javascript-sdk/.env
 *
 * If RUSHDB_API_KEY is absent the whole suite is skipped gracefully.
 */

import RushDB from '../../packages/javascript-sdk/src/index.node'

jest.setTimeout(60_000)

const apiKey = process.env.RUSHDB_API_KEY
const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

if (!apiKey) {
  describe('records.find() – traversal filter + pagination (e2e)', () => {
    it('skips because RUSHDB_API_KEY is not set', () => expect(true).toBe(true))
  })
} else {
  const db = new RushDB(apiKey, { url: apiUrl })

  const tenantId = `where-nested-pg-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

  const L_PARENT = 'PGParent'
  const L_CHILD = 'PGChild'

  const CHILDREN_PER_PARENT = 150

  beforeAll(async () => {
    await db.records.delete({ where: { tenantId } }).catch(() => {})

    await db.records.importJson({
      label: L_PARENT,
      data: [
        {
          name: 'A',
          tenantId,
          [L_CHILD]: Array.from({ length: CHILDREN_PER_PARENT }, (_, i) => ({ seq: i, tenantId }))
        },
        {
          name: 'B',
          tenantId,
          [L_CHILD]: Array.from({ length: CHILDREN_PER_PARENT }, (_, i) => ({
            seq: i + CHILDREN_PER_PARENT,
            tenantId
          }))
        }
      ]
    })
  })

  afterAll(async () => {
    await db.records.delete({ where: { tenantId } }).catch(() => {})
  })

  describe('records.find() – traversal filter + pagination (e2e)', () => {
    it('reports the correct total for a related-label filter beyond the first page', async () => {
      const res = await db.records.find({
        labels: [L_CHILD],
        where: { tenantId, [L_PARENT]: { name: 'B' } },
        orderBy: { seq: 'asc' },
        limit: 100
      })
      expect(res.total).toBe(CHILDREN_PER_PARENT)
    })

    it('returns a full first page of matches for a parent whose children sit outside the raw scan window', async () => {
      const res = await db.records.find({
        labels: [L_CHILD],
        where: { tenantId, [L_PARENT]: { name: 'B' } },
        orderBy: { seq: 'asc' },
        limit: 100
      })

      expect(res.data).toHaveLength(100)
      // Every returned record must actually belong to parent B (seq >= 150),
      // never a record from A that leaked in from the unfiltered window.
      expect((res.data as any[]).every((r) => r.data.seq >= CHILDREN_PER_PARENT)).toBe(true)
    })

    it('returns the remaining matches on the second page', async () => {
      const res = await db.records.find({
        labels: [L_CHILD],
        where: { tenantId, [L_PARENT]: { name: 'B' } },
        orderBy: { seq: 'asc' },
        skip: 100,
        limit: 100
      })

      expect(res.data).toHaveLength(50)
      expect((res.data as any[]).every((r) => r.data.seq >= CHILDREN_PER_PARENT)).toBe(true)
    })

    it('still paginates correctly for a parent whose children DO sit inside the first window (regression sanity check)', async () => {
      const res = await db.records.find({
        labels: [L_CHILD],
        where: { tenantId, [L_PARENT]: { name: 'A' } },
        orderBy: { seq: 'asc' },
        limit: 100
      })

      expect(res.total).toBe(CHILDREN_PER_PARENT)
      expect(res.data).toHaveLength(100)
      expect((res.data as any[]).every((r) => r.data.seq < CHILDREN_PER_PARENT)).toBe(true)
    })
  })
}
