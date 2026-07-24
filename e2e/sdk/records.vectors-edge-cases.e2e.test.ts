/**
 * Edge-case coverage for record vector operations not tested elsewhere.
 *
 * Tests:
 *   1. Embedding index creation WITHOUT any prior records — property node
 *      doesn't exist yet, so type validation is skipped (not required).
 *   2. record.score getter on vector search results — must shadow data.__score
 *      and be undefined on records from regular find().
 *   3. createMany with flat VectorEntry[] — ergonomic single-level list
 *      auto-wrapped to per-record VectorEntry[][] on the backend call.
 *
 * All tests use external indexes + inline vectors to avoid depending on
 * server-side embedding providers.
 */

import RushDB from '../../packages/javascript-sdk/src/index.node'
import type { EmbeddingIndex } from '../../packages/javascript-sdk/src/api/types'

jest.setTimeout(120_000)

const unitVec = (i: number, dims = 3): number[] => Array.from({ length: dims }, (_, k) => (k === i ? 1 : 0))

describe('records — vector edge cases (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => expect(true).toBe(true))
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })

  const tenantId = `vec-edge-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  const LABEL = 'VectorEdgeArticle'
  const PROP = 'content'
  const DIMS = 3

  let indexId: string | undefined

  // ── Setup ────────────────────────────────────────────────────────────────

  beforeAll(async () => {
    await db.records.delete({ labels: [LABEL] }).catch(() => {})
    const { data: allIndexes } = await db.ai.indexes.find()
    await Promise.all(
      (allIndexes as EmbeddingIndex[])
        .filter((i) => i.label === LABEL)
        .map((i) => db.ai.indexes.delete(i.id).catch(() => {}))
    )
  })

  afterAll(async () => {
    await db.records.delete({ labels: [LABEL], where: { tenantId } }).catch(() => {})
    if (indexId) await db.ai.indexes.delete(indexId).catch(() => {})
  })

  // ═══════════════════════════════════════════════════════════════════════
  //  1. Index creation without prior records
  //     The property node doesn't exist yet — the server skips type
  //     validation and creates the index purely as metadata.
  // ═══════════════════════════════════════════════════════════════════════

  describe('ai.indexes.create — cold start (no prior data)', () => {
    it('creates an external index when no record with that property exists', async () => {
      const { data: idx } = await db.ai.indexes.create({
        label: LABEL,
        propertyName: PROP,
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: DIMS
      })

      indexId = idx.id

      expect(idx).toBeDefined()
      expect(idx.label).toBe(LABEL)
      expect(idx.propertyName).toBe(PROP)
      expect(idx.sourceType).toBe('external')
      expect(idx.dimensions).toBe(DIMS)
      expect(idx.status).toBe('awaiting_vectors')
    })

    it('creates second index on a different property, also cold', async () => {
      const { data: idx } = await db.ai.indexes.create({
        label: LABEL,
        propertyName: 'summary',
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: DIMS
      })

      expect(idx).toBeDefined()
      expect(idx.status).toBe('awaiting_vectors')

      await db.ai.indexes.delete(idx.id)
    })
  })

  // ═══════════════════════════════════════════════════════════════════════
  //  2. record.score getter
  //     Convenience property on DBRecordInstance that shadows data.__score.
  // ═══════════════════════════════════════════════════════════════════════

  describe('DBRecordInstance.score — getter for vector search scores', () => {
    it('returns score via getter after vector search', async () => {
      const record = await db.records.create({
        label: LABEL,
        data: { content: 'vector search target', tenantId },
        vectors: [{ propertyName: PROP, vector: unitVec(0) }]
      })

      const res = await db.records.vectorSearch({
        propertyName: PROP,
        labels: [LABEL],
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: DIMS,
        queryVector: unitVec(0),
        where: { tenantId },
        limit: 5
      })

      expect(res.data.length).toBeGreaterThan(0)

      const hit = res.data[0]
      expect(hit.score).toBeDefined()
      expect(typeof hit.score).toBe('number')
      expect(hit.score).toBeGreaterThan(0)
      expect(hit.score).toBeLessThanOrEqual(1)
    })

    it('record.score matches data.__score', async () => {
      const res = await db.records.vectorSearch({
        propertyName: PROP,
        labels: [LABEL],
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: DIMS,
        queryVector: unitVec(0),
        where: { tenantId },
        limit: 5
      })

      for (const hit of res.data) {
        expect(hit.score).toBe(hit.data.__score)
      }
    })

    it('record.score is undefined on records from regular find (not vector search)', async () => {
      const res = await db.records.find({ labels: [LABEL], where: { tenantId } })
      if (res.data.length > 0) {
        for (const record of res.data) {
          expect(record.score).toBeUndefined()
        }
      }
    })

    it('record.score is typed as number | undefined', async () => {
      const res = await db.records.vectorSearch({
        propertyName: PROP,
        labels: [LABEL],
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: DIMS,
        queryVector: unitVec(0),
        where: { tenantId },
        limit: 5
      })

      for (const hit of res.data) {
        const s: number | undefined = hit.score
        expect(s).toEqual(hit.data.__score)
      }
    })
  })

  // ═══════════════════════════════════════════════════════════════════════
  //  3. createMany with flat VectorEntry[]
  //     Accepts VectorEntry[] | VectorEntry[][], auto-wrapping flat lists.
  // ═══════════════════════════════════════════════════════════════════════

  describe('records.createMany — vectors param overloads', () => {
    it('accepts flat VectorEntry[] (auto-wraps to per-record)', async () => {
      const records = await db.records.createMany({
        label: LABEL,
        data: [
          { content: 'flat vec alpha', tenantId },
          { content: 'flat vec beta', tenantId }
        ],
        options: { returnResult: true },
        vectors: [
          { propertyName: PROP, vector: unitVec(0) },
          { propertyName: PROP, vector: unitVec(1) }
        ]
      })

      expect(records.data.length).toBe(2)
    })

    it('still accepts nested VectorEntry[][] (backward compat)', async () => {
      const records = await db.records.createMany({
        label: LABEL,
        data: [
          { content: 'nested vec alpha', tenantId },
          { content: 'nested vec beta', tenantId }
        ],
        options: { returnResult: true },
        vectors: [
          [{ propertyName: PROP, vector: unitVec(0) }],
          [{ propertyName: PROP, vector: unitVec(1) }]
        ]
      })

      expect(records.data.length).toBe(2)
    })

    it('vectors written via flat VectorEntry[] are searchable', async () => {
      const res = await db.records.vectorSearch({
        propertyName: PROP,
        labels: [LABEL],
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: DIMS,
        queryVector: unitVec(0),
        where: { tenantId, content: 'flat vec alpha' },
        limit: 5
      })

      expect(res.data.length).toBeGreaterThan(0)
    })
  })
})
