/**
 * E2E tests for the inline-vector BYOV (Bring Your Own Vectors) DX.
 *
 * This file exercises every surface area of the inline-vector feature:
 *   - records.create()   with vectors: [...]
 *   - records.upsert()   with vectors: [...]
 *   - records.set()      with vectors: [...]
 *   - records.importJson with $vectors per item
 *   - ai.indexes.create() with external: true shorthand
 *   - ai.search()        with queryVector (no query text)
 *   - disambiguation: two indexes on same property, different similarityFunction
 *   - error paths: wrong dimensions, ambiguous match, no matching index
 *
 * Prerequisites
 * ─────────────
 * RUSHDB_API_KEY and RUSHDB_API_URL must be set in packages/javascript-sdk/.env
 *
 * If RUSHDB_API_KEY is absent the whole suite is skipped gracefully.
 */

import path from 'path'
import dotenv from 'dotenv'

dotenv.config({ path: path.resolve(__dirname, '../.env') })

import RushDB from '../src/index.node'
import type { EmbeddingIndex } from '../src/api/types'

jest.setTimeout(120_000)

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/** Wait for an embedding index to reach 'ready', polling every 2 s. */
async function waitForIndexReady(
  db: RushDB,
  indexId: string,
  timeoutMs = 60_000,
  interval = 2_000
): Promise<void> {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    const list = await db.ai.indexes.find()
    const idx = (list.data as EmbeddingIndex[]).find((i) => i.id === indexId)
    if (idx?.status === 'ready') return
    if (idx?.status === 'error') throw new Error(`Index ${indexId} entered error state`)
    await new Promise((r) => setTimeout(r, interval))
  }
  throw new Error(`Index ${indexId} did not become ready within ${timeoutMs} ms`)
}

/** Build a 3-D unit cosine vector pointing at slot `i` (0-indexed). */
const unitVec = (i: number, dims = 3): number[] => Array.from({ length: dims }, (_, k) => (k === i ? 1 : 0))

// ─────────────────────────────────────────────────────────────────────────────
// Suite
// ─────────────────────────────────────────────────────────────────────────────

describe('ai – inline vectors BYOV (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => expect(true).toBe(true))
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })

  // Unique tenant tag so parallel test runs never collide
  const tenantId = `inline-vec-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  const LABEL = 'InlineVecArticle'
  const PROP = 'body'
  const DIMS = 3

  // Track created index IDs for cleanup
  const createdIndexIds: string[] = []

  // Helper: create external index scoped to LABEL:PROP with DIMS dimensions
  async function makeIndex(similarityFunction: 'cosine' | 'euclidean' = 'cosine') {
    const { data: idx } = await db.ai.indexes.create({
      label: LABEL,
      propertyName: PROP,
      external: true, // shorthand instead of sourceType: 'external'
      similarityFunction,
      dimensions: DIMS
    })
    createdIndexIds.push(idx.id)
    return idx
  }

  // ── Teardown ────────────────────────────────────────────────────────────────

  afterAll(async () => {
    await db.records.delete({ labels: [LABEL], where: { tenantId } }).catch(() => {})
    for (const id of createdIndexIds) {
      await db.ai.indexes.delete(id).catch(() => {})
    }
  })

  // ═══════════════════════════════════════════════════════════════════════════
  // 1. Index creation via `external: true` shorthand
  // ═══════════════════════════════════════════════════════════════════════════

  describe('ai.indexes.create() with external: true', () => {
    it('creates an external index and resolves sourceType correctly', async () => {
      const { data: idx } = await db.ai.indexes.create({
        label: `${LABEL}Meta`,
        propertyName: 'summary',
        external: true,
        dimensions: DIMS,
        similarityFunction: 'cosine'
      })
      createdIndexIds.push(idx.id)

      expect(idx.sourceType).toBe('external')
      expect(idx.dimensions).toBe(DIMS)
      expect(idx.similarityFunction).toBe('cosine')
    })

    it('creates an external index via explicit sourceType (no regression)', async () => {
      const { data: idx } = await db.ai.indexes.create({
        label: `${LABEL}Compat`,
        propertyName: 'description',
        sourceType: 'external',
        dimensions: DIMS,
        similarityFunction: 'cosine'
      })
      createdIndexIds.push(idx.id)

      expect(idx.sourceType).toBe('external')
    })
  })

  // ═══════════════════════════════════════════════════════════════════════════
  // 2. records.create() with inline vectors
  // ═══════════════════════════════════════════════════════════════════════════

  describe('records.create() with vectors', () => {
    let indexId: string

    beforeAll(async () => {
      const idx = await makeIndex('cosine')
      indexId = idx.id
    })

    it('creates a record and writes the inline vector in a single call', async () => {
      const record = await db.records.create({
        label: LABEL,
        data: { body: 'Alpha article', tenantId },
        vectors: [{ propertyName: PROP, vector: unitVec(0) }]
      })

      expect(record.id).toBeDefined()
      expect(record.data.body).toBe('Alpha article')

      // Verify the vector was written: search with the same vector → should return this record
      await waitForIndexReady(db, indexId)

      const res = await db.ai.search({
        propertyName: PROP,
        labels: [LABEL],
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: DIMS,
        queryVector: unitVec(0),
        where: { tenantId, body: 'Alpha article' },
        limit: 5
      })

      expect(res.success).toBe(true)
      expect(res.data.length).toBeGreaterThan(0)
      const top = res.data[0]
      expect(top.__id).toBe(record.id)
      expect(top.__score).toBeCloseTo(1, 2)
    })

    it('returns results in descending __score order', async () => {
      // Create two more records with distinct unit vectors
      await db.records.create({
        label: LABEL,
        data: { body: 'Beta article', tenantId },
        vectors: [{ propertyName: PROP, vector: unitVec(1) }]
      })
      await db.records.create({
        label: LABEL,
        data: { body: 'Gamma article', tenantId },
        vectors: [{ propertyName: PROP, vector: unitVec(2) }]
      })

      await waitForIndexReady(db, indexId)

      const res = await db.ai.search({
        propertyName: PROP,
        labels: [LABEL],
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: DIMS,
        queryVector: unitVec(0),
        where: { tenantId },
        limit: 10
      })

      expect(res.data.length).toBeGreaterThan(1)
      for (let i = 0; i < res.data.length - 1; i++) {
        expect(res.data[i].__score).toBeGreaterThanOrEqual(res.data[i + 1].__score)
      }
    })
  })

  // ═══════════════════════════════════════════════════════════════════════════
  // 3. records.upsert() with inline vectors
  // ═══════════════════════════════════════════════════════════════════════════

  describe('records.upsert() with vectors', () => {
    let indexId: string

    beforeAll(async () => {
      const idx = await makeIndex('cosine')
      indexId = idx.id
    })

    it('creates a record and writes a vector on first upsert', async () => {
      const upsertTenant = `${tenantId}-upsert`

      const record = await db.records.upsert({
        label: LABEL,
        data: { body: 'Upsert Alpha', slug: 'u-alpha', tenantId: upsertTenant },
        options: { mergeBy: ['slug', 'tenantId'], suggestTypes: true },
        vectors: [{ propertyName: PROP, vector: unitVec(0) }]
      })

      expect(record.id).toBeDefined()
      await waitForIndexReady(db, indexId)

      const res = await db.ai.search({
        propertyName: PROP,
        labels: [LABEL],
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: DIMS,
        queryVector: unitVec(0),
        where: { tenantId: upsertTenant },
        limit: 5
      })

      expect(res.data[0]?.__id).toBe(record.id)
    })

    it('updates the vector when the same record is upserted again', async () => {
      const upsertTenant = `${tenantId}-upsert2`

      // First upsert → vector points at slot 1
      const first = await db.records.upsert({
        label: LABEL,
        data: { body: 'Update vector article', slug: 'u-update', tenantId: upsertTenant },
        options: { mergeBy: ['slug', 'tenantId'], suggestTypes: true },
        vectors: [{ propertyName: PROP, vector: unitVec(1) }]
      })

      // Second upsert same slug → vector should now point at slot 0
      const second = await db.records.upsert({
        label: LABEL,
        data: { body: 'Update vector article', slug: 'u-update', tenantId: upsertTenant },
        options: { mergeBy: ['slug', 'tenantId'], suggestTypes: true },
        vectors: [{ propertyName: PROP, vector: unitVec(0) }]
      })

      // Both upserts should resolve to the same record
      expect(second.id).toBe(first.id)

      await waitForIndexReady(db, indexId)

      // Search with slot-0 → should be the top result (score ≈ 1)
      const res = await db.ai.search({
        propertyName: PROP,
        labels: [LABEL],
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: DIMS,
        queryVector: unitVec(0),
        where: { tenantId: upsertTenant },
        limit: 5
      })

      expect(res.data[0]?.__id).toBe(first.id)
      expect(res.data[0]?.__score).toBeCloseTo(1, 2)
    })
  })

  // ═══════════════════════════════════════════════════════════════════════════
  // 4. records.set() with inline vectors
  // ═══════════════════════════════════════════════════════════════════════════

  describe('records.set() with vectors', () => {
    let indexId: string

    beforeAll(async () => {
      const idx = await makeIndex('cosine')
      indexId = idx.id
    })

    it('overwrites record data and writes the inline vector atomically', async () => {
      const setTenant = `${tenantId}-set`

      // Create without vector first
      const original = await db.records.create({
        label: LABEL,
        data: { body: 'Old body', extra: 'keep', tenantId: setTenant }
      })

      // Set replaces data and writes vector
      const updated = await db.records.set({
        target: original,
        label: LABEL,
        data: { body: 'New body', tenantId: setTenant },
        vectors: [{ propertyName: PROP, vector: unitVec(0) }]
      })

      expect(updated.id).toBe(original.id)
      expect(updated.data.body).toBe('New body')

      await waitForIndexReady(db, indexId)

      const res = await db.ai.search({
        propertyName: PROP,
        labels: [LABEL],
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: DIMS,
        queryVector: unitVec(0),
        where: { tenantId: setTenant },
        limit: 5
      })

      expect(res.data[0]?.__id).toBe(original.id)
    })
  })

  // ═══════════════════════════════════════════════════════════════════════════
  // 5. records.importJson() with $vectors
  // ═══════════════════════════════════════════════════════════════════════════

  describe('records.importJson() with $vectors', () => {
    let indexId: string

    beforeAll(async () => {
      const idx = await makeIndex('cosine')
      indexId = idx.id
    })

    it('writes vectors for each item in a batch import', async () => {
      const importTenant = `${tenantId}-import`

      await db.records.importJson({
        label: LABEL,
        data: [
          {
            body: 'Import Alpha',
            tenantId: importTenant,
            $vectors: [{ propertyName: PROP, vector: unitVec(0) }]
          },
          {
            body: 'Import Beta',
            tenantId: importTenant,
            $vectors: [{ propertyName: PROP, vector: unitVec(1) }]
          },
          {
            body: 'Import Gamma',
            tenantId: importTenant,
            $vectors: [{ propertyName: PROP, vector: unitVec(2) }]
          }
        ]
      })

      await waitForIndexReady(db, indexId)

      // Query close to slot 0 → Import Alpha should rank highest
      const res = await db.ai.search({
        propertyName: PROP,
        labels: [LABEL],
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: DIMS,
        queryVector: unitVec(0),
        where: { tenantId: importTenant },
        limit: 5
      })

      expect(res.data.length).toBe(3)
      expect(String(res.data[0].body ?? '')).toBe('Import Alpha')
      expect(res.data[0].__score).toBeCloseTo(1, 2)
    })

    it('does not create $vectors as child records', async () => {
      // If $vectors were processed by BFS as child entities, we'd find InlineVecArticle-like
      // child records. Verify they don't exist.
      const importTenant = `${tenantId}-no-children`

      await db.records.importJson({
        label: LABEL,
        data: [
          {
            body: 'No children check',
            tenantId: importTenant,
            $vectors: [{ propertyName: PROP, vector: unitVec(0) }]
          }
        ]
      })

      // Only one record should exist (the parent), none named after $vectors keys
      const all = await db.records.find({
        labels: [LABEL],
        where: { tenantId: importTenant }
      })

      expect(all.total).toBe(1)
    })
  })

  // ═══════════════════════════════════════════════════════════════════════════
  // 6. Semantic search: dimension auto-inference from queryVector.length
  // ═══════════════════════════════════════════════════════════════════════════

  describe('ai.search() dimension inference', () => {
    let indexId: string

    beforeAll(async () => {
      const idx = await makeIndex('cosine')
      indexId = idx.id
    })

    it('finds the index when dimensions is omitted (inferred from queryVector length)', async () => {
      const inferTenant = `${tenantId}-infer`

      await db.records.create({
        label: LABEL,
        data: { body: 'Infer dims', tenantId: inferTenant },
        vectors: [{ propertyName: PROP, vector: unitVec(0) }]
      })

      await waitForIndexReady(db, indexId)

      // No `dimensions` field — server must infer 3 from queryVector.length
      const res = await db.ai.search({
        propertyName: PROP,
        labels: [LABEL],
        sourceType: 'external',
        similarityFunction: 'cosine',
        queryVector: unitVec(0), // length = 3
        where: { tenantId: inferTenant },
        limit: 5
      })

      expect(res.success).toBe(true)
      expect(res.data.length).toBeGreaterThan(0)
    })
  })

  // ═══════════════════════════════════════════════════════════════════════════
  // 7. Disambiguation: two indexes share same property, different similarityFunction
  // ═══════════════════════════════════════════════════════════════════════════

  describe('disambiguation via similarityFunction', () => {
    const DISAMLABEL = `${LABEL}Disambig`
    const DISAMTENANTID = `${tenantId}-disambig`
    let cosineIndexId: string
    let euclideanIndexId: string

    beforeAll(async () => {
      // Create two indexes on the same property but different similarity functions
      const [{ data: ci }, { data: ei }] = await Promise.all([
        db.ai.indexes.create({
          label: DISAMLABEL,
          propertyName: PROP,
          external: true,
          dimensions: DIMS,
          similarityFunction: 'cosine'
        }),
        db.ai.indexes.create({
          label: DISAMLABEL,
          propertyName: PROP,
          external: true,
          dimensions: DIMS,
          similarityFunction: 'euclidean'
        })
      ])
      cosineIndexId = ci.id
      euclideanIndexId = ei.id
      createdIndexIds.push(cosineIndexId, euclideanIndexId)
    })

    afterAll(async () => {
      await db.records.delete({ labels: [DISAMLABEL], where: { tenantId: DISAMTENANTID } }).catch(() => {})
    })

    it('writes to the correct index when similarityFunction is specified', async () => {
      // This should succeed — exactly one cosine index matches
      const record = await db.records.create({
        label: DISAMLABEL,
        data: { body: 'Disambig article', tenantId: DISAMTENANTID },
        vectors: [{ propertyName: PROP, vector: unitVec(0), similarityFunction: 'cosine' }]
      })

      expect(record.id).toBeDefined()
    })

    it('writes to the euclidean index when similarityFunction is euclidean', async () => {
      const record = await db.records.create({
        label: DISAMLABEL,
        data: { body: 'Euclidean article', tenantId: DISAMTENANTID },
        vectors: [{ propertyName: PROP, vector: unitVec(1), similarityFunction: 'euclidean' }]
      })

      expect(record.id).toBeDefined()
    })

    it('rejects inline vectors when similarityFunction is omitted (ambiguous match)', async () => {
      // Two indexes exist → server cannot pick one → must return error
      await expect(
        db.records.create({
          label: DISAMLABEL,
          data: { body: 'Ambiguous article', tenantId: DISAMTENANTID },
          vectors: [
            {
              propertyName: PROP,
              vector: unitVec(0)
              // no similarityFunction → ambiguous
            }
          ]
        })
      ).rejects.toBeTruthy()
    })
  })

  // ═══════════════════════════════════════════════════════════════════════════
  // 9. records.createMany() with vectors
  // ═══════════════════════════════════════════════════════════════════════════

  describe('records.createMany() with vectors', () => {
    let indexId: string

    beforeAll(async () => {
      const idx = await makeIndex('cosine')
      indexId = idx.id
    })

    it('creates multiple records and writes per-row vectors', async () => {
      const rows = [
        { body: 'createMany alpha', tenantId },
        { body: 'createMany beta', tenantId },
        { body: 'createMany gamma', tenantId }
      ]

      const result = await db.records.createMany({
        label: LABEL,
        data: rows,
        options: { returnResult: true },
        vectors: [
          [{ propertyName: PROP, vector: unitVec(0) }], // row 0 → [1,0,0]
          [{ propertyName: PROP, vector: unitVec(1) }], // row 1 → [0,1,0]
          [{ propertyName: PROP, vector: unitVec(2) }] // row 2 → [0,0,1]
        ]
      })

      expect(result.data.length).toBe(3)

      // Index [1,0,0] — should find row 0 at score ≈ 1
      await waitForIndexReady(db, indexId)
      const { data: res } = await db.ai.search({
        labels: [LABEL],
        propertyName: PROP,
        queryVector: unitVec(0),
        where: { tenantId },
        limit: 3
      })
      expect(res.length).toBeGreaterThanOrEqual(1)
      expect(res[0].__score).toBeGreaterThan(0.99)
    })

    it('rejects when vectors length exceeds data length (client-side)', async () => {
      expect(() =>
        db.records.createMany({
          label: LABEL,
          data: [{ body: 'one row', tenantId }],
          vectors: [
            [{ propertyName: PROP, vector: unitVec(0) }],
            [{ propertyName: PROP, vector: unitVec(1) }] // extra — no corresponding row
          ]
        })
      ).toThrow(/vectors length.*exceeds/)
    })

    it('allows sparse vectors — only some rows need vectors', async () => {
      const rows = [
        { body: 'createMany sparse 0', tenantId },
        { body: 'createMany sparse 1', tenantId },
        { body: 'createMany sparse 2', tenantId }
      ]

      // Only provide a vector for row 0; rows 1 and 2 are left without vectors
      const result = await db.records.createMany({
        label: LABEL,
        data: rows,
        options: { returnResult: true },
        vectors: [[{ propertyName: PROP, vector: unitVec(0) }]]
      })

      expect(result.data.length).toBe(3)
    })
  })

  // ═══════════════════════════════════════════════════════════════════════════
  // 10. records.importCsv() with vectors
  // ═══════════════════════════════════════════════════════════════════════════

  describe('records.importCsv() with vectors', () => {
    let indexId: string

    beforeAll(async () => {
      const idx = await makeIndex('cosine')
      indexId = idx.id
    })

    // Build CSV with tenantId baked in
    function makeCsv() {
      return (
        `body,tenantId\n` +
        `importCsv alpha,${tenantId}\n` +
        `importCsv beta,${tenantId}\n` +
        `importCsv gamma,${tenantId}`
      )
    }

    it('imports CSV rows and writes per-row vectors', async () => {
      const result = await db.records.importCsv({
        label: LABEL,
        data: makeCsv(),
        options: { returnResult: true },
        vectors: [
          [{ propertyName: PROP, vector: unitVec(0) }], // row 0 → [1,0,0]
          [{ propertyName: PROP, vector: unitVec(1) }], // row 1 → [0,1,0]
          [{ propertyName: PROP, vector: unitVec(2) }] // row 2 → [0,0,1]
        ]
      })

      expect(result.data.length).toBe(3)

      // Vector [1,0,0] — row 0 should be closest
      await waitForIndexReady(db, indexId)
      const { data: res } = await db.ai.search({
        labels: [LABEL],
        propertyName: PROP,
        queryVector: unitVec(0),
        where: { tenantId },
        limit: 3
      })
      expect(res.length).toBeGreaterThanOrEqual(1)
      expect(res[0].__score).toBeGreaterThan(0.99)
    })

    it('rejects when vectors length exceeds number of CSV rows (server-side)', async () => {
      // Only 3 data rows but we provide 5 vector entries
      await expect(
        db.records.importCsv({
          label: LABEL,
          data: makeCsv(),
          vectors: [
            [{ propertyName: PROP, vector: unitVec(0) }],
            [{ propertyName: PROP, vector: unitVec(1) }],
            [{ propertyName: PROP, vector: unitVec(2) }],
            [{ propertyName: PROP, vector: unitVec(0) }], // no row 3
            [{ propertyName: PROP, vector: unitVec(1) }] // no row 4
          ]
        })
      ).rejects.toBeTruthy()
    })

    it('allows sparse vectors — only some CSV rows need vectors', async () => {
      const result = await db.records.importCsv({
        label: LABEL,
        data: makeCsv(),
        options: { returnResult: true },
        vectors: [
          [{ propertyName: PROP, vector: unitVec(2) }] // only row 0 has a vector
        ]
      })
      expect(result.data.length).toBe(3)
    })
  })

  // ═══════════════════════════════════════════════════════════════════════════
  // 8. Error paths
  // ═══════════════════════════════════════════════════════════════════════════

  describe('error paths', () => {
    let indexId: string

    beforeAll(async () => {
      const idx = await makeIndex('cosine')
      indexId = idx.id
    })

    it('rejects a vector with wrong dimensions', async () => {
      await expect(
        db.records.create({
          label: LABEL,
          data: { body: 'Wrong dims', tenantId },
          vectors: [
            {
              propertyName: PROP,
              vector: [0.1, 0.2, 0.3, 0.4] // 4 dims, index expects 3
            }
          ]
        })
      ).rejects.toBeTruthy()
    })

    it('rejects a vector when no matching external index exists', async () => {
      await expect(
        db.records.create({
          label: LABEL,
          data: { body: 'No index', tenantId },
          vectors: [
            {
              propertyName: 'nonExistentProp', // no index on this property
              vector: unitVec(0)
            }
          ]
        })
      ).rejects.toBeTruthy()
    })

    it('rejects query text against an external index during search', async () => {
      await expect(
        db.ai.search({
          propertyName: PROP,
          labels: [LABEL],
          sourceType: 'external',
          similarityFunction: 'cosine',
          dimensions: DIMS,
          query: 'query text is not allowed for external indexes',
          where: { tenantId },
          limit: 5
        })
      ).rejects.toBeTruthy()
    })
  })
})
