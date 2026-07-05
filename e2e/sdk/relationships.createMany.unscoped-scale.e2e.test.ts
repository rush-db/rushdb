import RushDB from '../../packages/javascript-sdk/src/index.node'

const sleep = (ms: number) => new Promise((res) => setTimeout(res, ms))

/**
 * Regression suite for the client-reported quadratic scan on the bulk
 * relationship endpoints: an UNSCOPED source side used to re-match the target
 * label once per source record, so ~2,100 source records timed out at the 60s
 * gateway budget even when only a handful of pairs were written. The join is
 * now a single-pass hash join — cost tracks matched pairs, not label size.
 *
 * Mirrors the reported dataset shape: ~2,100 source records joined to targets
 * on a scalar string key, target side scoped with `$in`, source side unscoped.
 */
describe('relationships.createMany/deleteMany with unscoped source at scale (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: process.env.RUSHDB_API_URL })

  // Unique labels per run make the unscoped-source shape safe to exercise.
  const runId = `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`
  const SOURCE = `SCALE_SRC_${runId}`
  const TARGET = `SCALE_TGT_${runId}`
  const REL = 'SCALE_REL'

  const SOURCE_COUNT = 2100
  const TARGET_COUNT = 600 // every 3rd source key is referenced by exactly one target

  // Generous bound: the old per-source re-scan took tens of seconds at this
  // scale even on local hardware; the hash join takes ~1-2s.
  const TIME_BUDGET_MS = 20_000

  const sourceData = Array.from({ length: SOURCE_COUNT }, (_, i) => ({ key: `k${i}` }))
  const targetData = Array.from({ length: TARGET_COUNT }, (_, i) => ({ key: `k${i * 3}` }))
  const scopedTargetKeys = targetData.slice(0, 100).map((t) => t.key)

  jest.setTimeout(180_000)

  const countRelations = async (): Promise<number> => {
    const res = await db.relationships.find({
      source: { labels: [SOURCE] },
      target: { labels: [TARGET] },
      where: { type: REL },
      limit: 1000,
      skip: 0
    })
    expect(res.success).toBe(true)
    return (res.data || []).length
  }

  const waitForRelationCount = async (expected: number): Promise<number> => {
    let count = -1
    for (let attempts = 0; attempts < 8; attempts++) {
      count = await countRelations()
      if (count === expected) return count
      await sleep(1000)
    }
    return count
  }

  beforeAll(async () => {
    await db.records.createMany({ label: SOURCE, data: sourceData, options: { returnResult: false } })
    await db.records.createMany({ label: TARGET, data: targetData, options: { returnResult: false } })
  })

  it('key-joins an unscoped source against a scoped target in seconds, writing only matched pairs', async () => {
    const startedAt = Date.now()
    const res = await db.relationships.createMany({
      source: { label: SOURCE, key: 'key' }, // no `where` — the reported hot shape
      target: { label: TARGET, key: 'key', where: { key: { $in: scopedTargetKeys } } },
      type: REL,
      direction: 'out'
    })
    const elapsed = Date.now() - startedAt

    console.info(`[scale] createMany unscoped source (${SOURCE_COUNT} records, 100 target keys): ${elapsed}ms`)
    expect(res.success).toBe(true)
    expect(elapsed).toBeLessThan(TIME_BUDGET_MS)
    expect(await waitForRelationCount(scopedTargetKeys.length)).toBe(scopedTargetKeys.length)
  })

  it('deletes through the same unscoped-source shape in seconds', async () => {
    const startedAt = Date.now()
    const res = await db.relationships.deleteMany({
      source: { label: SOURCE, key: 'key' },
      target: { label: TARGET, key: 'key', where: { key: { $in: scopedTargetKeys } } },
      type: REL,
      direction: 'out'
    })
    const elapsed = Date.now() - startedAt

    console.info(`[scale] deleteMany unscoped source: ${elapsed}ms`)
    expect(res.success).toBe(true)
    expect(elapsed).toBeLessThan(TIME_BUDGET_MS)
    expect(await waitForRelationCount(0)).toBe(0)
  })

  it('allows a fully unscoped key join (manyToMany flag set) and links every referenced source', async () => {
    const startedAt = Date.now()
    const res = await db.relationships.createMany({
      source: { label: SOURCE, key: 'key' },
      target: { label: TARGET, key: 'key' },
      type: REL,
      direction: 'out',
      manyToMany: true // used to be rejected without `where` on both sides; a key join is not cartesian
    })
    const elapsed = Date.now() - startedAt

    console.info(`[scale] createMany fully unscoped key join (${TARGET_COUNT} pairs): ${elapsed}ms`)
    expect(res.success).toBe(true)
    expect(elapsed).toBeLessThan(TIME_BUDGET_MS)
    expect(await waitForRelationCount(TARGET_COUNT)).toBe(TARGET_COUNT)

    const cleanup = await db.relationships.deleteMany({
      source: { label: SOURCE, key: 'key' },
      target: { label: TARGET, key: 'key' },
      type: REL,
      direction: 'out'
    })
    expect(cleanup.success).toBe(true)
    expect(await waitForRelationCount(0)).toBe(0)
  })

  it('still rejects a cartesian manyToMany missing where filters (one-sided key is not a join)', async () => {
    await expect(
      db.relationships.createMany({
        source: { label: SOURCE, key: 'key' }, // key on one side only → not a key join
        target: { label: TARGET, where: { key: 'k0' } },
        type: REL,
        direction: 'out',
        manyToMany: true
      })
    ).rejects.toThrow(/non-empty `where` filters|cartesian/i)
  })
})
