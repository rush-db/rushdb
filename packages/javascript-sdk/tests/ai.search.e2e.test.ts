/**
 * E2E tests for the semantic (vector) search flow via db.ai.search().
 *
 * Prerequisites
 * ─────────────
 * 1. RUSHDB_API_KEY and RUSHDB_API_URL must be set in packages/javascript-sdk/.env
 * 2. The server must be configured with:
 *      RUSHDB_EMBEDDING_MODEL, RUSHDB_EMBEDDING_DIMENSIONS, RUSHDB_EMBEDDING_API_KEY
 *
 * If any of these are missing, all tests in this file are skipped gracefully.
 *
 * HOW THE FLOW WORKS
 * ──────────────────
 * Indexes are scoped to (label, propertyName). "Book:title" and "Task:title" are completely
 * independent — they backfill only matching records and store rel.__propKey = "Book:title" /
 * "Task:title" on the VALUE relationships. The shared Neo4j DDL index
 * (`rushdb_emb_value_rels`) is created idempotently and dropped only when zero embeddings
 * remain anywhere in the graph.
 *
 * SEARCH QUERY SHAPE
 * ──────────────────
 * await db.ai.search({
 *   propertyName: 'description',  // which indexed property
 *   query:        'deep learning', // free-text — server embeds it
 *   labels:       ['Article'],     // required; first entry selects the index
 *   topK:         50,              // direct vector-index candidate pool (ignored in prefilter mode)
 *   limit:        10,
 *   skip:         0,
 *   // Adding `where` switches to prefilter mode:
 *   where: { published: true }
 * })
 *
 * RETURNED SHAPE
 * ──────────────
 * { data: Array<{ record: Record<string,unknown>, score: number }>, success: boolean }
 * score ∈ [0, 1] — cosine similarity, higher = more similar.
 */

import path from 'path'
import dotenv from 'dotenv'

dotenv.config({ path: path.resolve(__dirname, '../.env') })

import RushDB from '../src/index.node'
import type { EmbeddingIndex } from '../src/api/types'

jest.setTimeout(120_000) // embedding backfill can take a while

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/** Poll index status until 'ready' or timeout (ms). */
async function waitForIndexReady(
  db: RushDB,
  indexId: string,
  timeoutMs = 90_000,
  interval = 3_000
): Promise<void> {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    const list = await db.ai.indexes.find()
    const idx = (list.data as EmbeddingIndex[]).find((i) => i.id === indexId)
    if (idx?.status === 'ready') return
    if (idx?.status === 'error') throw new Error(`Embedding index ${indexId} entered error state`)
    await new Promise((r) => setTimeout(r, interval))
  }
  throw new Error(`Embedding index ${indexId} did not become ready within ${timeoutMs}ms`)
}

// ─────────────────────────────────────────────────────────────────────────────
// Test suite
// ─────────────────────────────────────────────────────────────────────────────

describe('db.ai.search – semantic (vector) search (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

  /** Skip the entire suite when credentials or embedding env vars are absent */
  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })

  // Unique tenant tag so test data never collides with other runs
  const tenantId = `ai-search-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  const LABEL = 'Article'
  const PROPERTY = 'description'

  /** Index ID captured after creation so it can be deleted in afterAll */
  let indexId: string | undefined

  // Sample corpus – each record has a distinct semantic theme
  const articles = [
    {
      title: 'Intro to Machine Learning',
      description:
        'Machine learning is a branch of artificial intelligence focused on building systems that learn from data.',
      category: 'ml',
      published: true,
      tenantId
    },
    {
      title: 'Deep Neural Networks',
      description:
        'Deep neural networks consist of multiple hidden layers that progressively extract higher-level features.',
      category: 'ml',
      published: true,
      tenantId
    },
    {
      title: 'Quantum Computing Basics',
      description:
        'Quantum computers use qubits and superposition to perform calculations impossible for classical computers.',
      category: 'quantum',
      published: true,
      tenantId
    },
    {
      title: 'French Cuisine',
      description:
        'French cuisine is celebrated worldwide for its refined techniques, rich sauces, and fresh ingredients.',
      category: 'food',
      published: false,
      tenantId
    },
    {
      title: 'Graph Database Internals',
      description:
        'Graph databases store data as nodes and edges, enabling efficient traversal of connected data structures.',
      category: 'databases',
      published: true,
      tenantId
    }
  ]

  // ── Setup / teardown ───────────────────────────────────────────────────────

  beforeAll(async () => {
    // Insert test records
    await db.records.createMany({
      label: LABEL,
      data: articles,
      options: { suggestTypes: true, returnResult: false }
    })

    // Create the embedding index policy scoped to (label, propertyName)
    let createdIndex: EmbeddingIndex
    try {
      const res = await db.ai.indexes.create({ label: LABEL, propertyName: PROPERTY })
      createdIndex = res.data as EmbeddingIndex
    } catch (err: any) {
      // 409 = index already exists (idempotent re-run) — fetch it
      const list = await db.ai.indexes.find()
      const existing = (list.data as EmbeddingIndex[]).find(
        (i) => i.propertyName === PROPERTY && i.label === LABEL
      )
      if (!existing) throw err
      createdIndex = existing
    }

    indexId = createdIndex.id

    // Wait until the backfill scheduler has embedded all values
    await waitForIndexReady(db, indexId)
  })

  afterAll(async () => {
    // Remove test data
    await db.records.delete({ labels: [LABEL], where: { tenantId } })
    // Remove the embedding index policy (best-effort)
    if (indexId) {
      try {
        await db.ai.indexes.delete(indexId)
      } catch {
        // ignore
      }
    }
  })

  // ── direct vector-index mode (no where, single label) ─────────────────────

  it('direct vector-index mode: returns semantically similar results for an ML query', async () => {
    const res = await db.ai.search({
      propertyName: PROPERTY,
      query: 'neural networks and artificial intelligence',
      labels: [LABEL],
      topK: 10,
      limit: 3
    })

    expect(res.success).toBe(true)
    expect(Array.isArray(res.data)).toBe(true)
    expect(res.data.length).toBeGreaterThan(0)
    expect(res.data.length).toBeLessThanOrEqual(3)

    // Every result must have a numeric score injected as __score
    res.data.forEach((item) => {
      expect(typeof item.__score).toBe('number')
      expect(item.__score).toBeGreaterThan(0)
      expect(item.__score).toBeLessThanOrEqual(1)
      expect(item.__id).toBeDefined()
    })

    // ML articles should rank higher than "French Cuisine" or "Quantum Computing"
    const topTitle = String(res.data[0].title ?? '')
    expect(['Intro to Machine Learning', 'Deep Neural Networks']).toContain(topTitle)
  })

  it('direct vector-index mode: returns results ordered by score descending', async () => {
    const res = await db.ai.search({
      propertyName: PROPERTY,
      query: 'graph database nodes edges',
      labels: [LABEL],
      topK: 10,
      limit: 5
    })

    expect(res.success).toBe(true)
    const scores = res.data.map((r) => r.__score)
    for (let i = 0; i < scores.length - 1; i++) {
      expect(scores[i]).toBeGreaterThanOrEqual(scores[i + 1])
    }
  })

  it('direct vector-index mode: respects limit and skip for pagination', async () => {
    const page1 = await db.ai.search({
      propertyName: PROPERTY,
      query: 'data science algorithms',
      labels: [LABEL],
      topK: 20,
      limit: 2,
      skip: 0
    })

    const page2 = await db.ai.search({
      propertyName: PROPERTY,
      query: 'data science algorithms',
      labels: [LABEL],
      topK: 20,
      limit: 2,
      skip: 2
    })

    expect(page1.success).toBe(true)
    expect(page2.success).toBe(true)

    // Pages must not overlap (different records)
    const ids1 = page1.data.map((r) => r.__id ?? r.title)
    const ids2 = page2.data.map((r) => r.__id ?? r.title)
    const overlap = ids1.filter((id) => ids2.includes(id))
    expect(overlap).toHaveLength(0)
  })

  // ── prefilter mode (where filter) ──────────────────────────────────────────

  it('prefilter mode: where filter restricts candidates before cosine scoring', async () => {
    // 'French Cuisine' has published: false — must not appear when filtering published: true
    const res = await db.ai.search({
      propertyName: PROPERTY,
      query: 'cooking and gastronomy',
      labels: [LABEL],
      where: { published: true, tenantId },
      limit: 5
    })

    expect(res.success).toBe(true)

    const titles = res.data.map((r) => String(r.title ?? ''))
    expect(titles).not.toContain('French Cuisine')
  })

  it('prefilter mode: where filter with $contains narrows the result set', async () => {
    const res = await db.ai.search({
      propertyName: PROPERTY,
      query: 'quantum superposition qubits',
      labels: [LABEL],
      where: { category: 'quantum', tenantId },
      limit: 5
    })

    expect(res.success).toBe(true)
    expect(res.data.length).toBeGreaterThan(0)

    // Every returned record must match the category filter
    res.data.forEach((item) => {
      expect(item.category).toBe('quantum')
    })
  })

  it('prefilter mode: multi-label array switches from direct vector-index mode to prefilter', async () => {
    // Two labels passed → prefilter mode even without a `where`
    const res = await db.ai.search({
      propertyName: PROPERTY,
      query: 'data and storage',
      labels: [LABEL, 'NonExistentLabel'],
      limit: 5
    })

    // Should still succeed; NonExistentLabel simply contributes zero candidates
    expect(res.success).toBe(true)
    expect(Array.isArray(res.data)).toBe(true)
  })

  // ── Error handling ─────────────────────────────────────────────────────────

  it('returns error when propertyName has no index', async () => {
    await expect(
      db.ai.search({
        propertyName: '__nonexistent_prop__',
        query: 'anything',
        labels: ['NonExistentLabel'],
        limit: 5
      })
    ).rejects.toThrow()
  })

  // ── Conflict guard ─────────────────────────────────────────────────────────

  it('returns 409 when creating a duplicate (label, propertyName) index', async () => {
    // Attempt to register the same index again; server must reject with 409
    await expect(db.ai.indexes.create({ label: LABEL, propertyName: PROPERTY })).rejects.toMatchObject({
      status: 409
    })
  })
})

// ─────────────────────────────────────────────────────────────────────────────
// Multi-label isolation
// ─────────────────────────────────────────────────────────────────────────────

describe('db.ai – multi-label index isolation (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => expect(true).toBe(true))
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })

  const tenantId = `ai-isolation-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  const PROPERTY = 'bio'
  const LABEL_A = 'Scientist'
  const LABEL_B = 'Chef'

  let indexAId: string | undefined
  let indexBId: string | undefined

  const scientists = [
    { bio: 'Pioneered the theory of general relativity and quantum mechanics.', tenantId },
    { bio: 'Developed the first programmable electronic computer and advanced computing theory.', tenantId },
    { bio: 'Discovered the structure of DNA using X-ray crystallography techniques.', tenantId }
  ]

  const chefs = [
    { bio: 'World-renowned for perfecting classic French cuisine and buttery sauces.', tenantId },
    { bio: 'A celebrated pastry chef known for innovative chocolate desserts.', tenantId },
    { bio: 'Specialises in fermentation, umami flavours, and Japanese kaiseki cuisine.', tenantId }
  ]

  beforeAll(async () => {
    // Insert records for both labels
    await Promise.all([
      db.records.createMany({
        label: LABEL_A,
        data: scientists,
        options: { suggestTypes: true, returnResult: false }
      }),
      db.records.createMany({
        label: LABEL_B,
        data: chefs,
        options: { suggestTypes: true, returnResult: false }
      })
    ])

    // Create separate indexes scoped to each label
    const [resA, resB] = await Promise.all([
      db.ai.indexes.create({ label: LABEL_A, propertyName: PROPERTY }),
      db.ai.indexes.create({ label: LABEL_B, propertyName: PROPERTY })
    ])
    indexAId = (resA.data as EmbeddingIndex).id
    indexBId = (resB.data as EmbeddingIndex).id

    // Wait for both to be ready
    await Promise.all([waitForIndexReady(db, indexAId!), waitForIndexReady(db, indexBId!)])
  })

  afterAll(async () => {
    await db.records.delete({ labels: [LABEL_A], where: { tenantId } }).catch(() => {})
    await db.records.delete({ labels: [LABEL_B], where: { tenantId } }).catch(() => {})
    if (indexAId) await db.ai.indexes.delete(indexAId).catch(() => {})
    if (indexBId) await db.ai.indexes.delete(indexBId).catch(() => {})
  })

  it('Scientist search does not return Chef records', async () => {
    const res = await db.ai.search({
      propertyName: PROPERTY,
      query: 'physics and relativity',
      labels: [LABEL_A],
      limit: 5
    })

    expect(res.success).toBe(true)
    expect(res.data.length).toBeGreaterThan(0)

    // All results must be Scientist records (i.e. bio topics are scientific)
    const bios = res.data.map((r) => String(r.bio ?? ''))
    bios.forEach((bio) => {
      // Chef bios contain food/cuisine keywords; none should appear
      expect(bio.toLowerCase()).not.toMatch(/cuisine|chocolate|dessert|fermentation|kaiseki|pastry/)
    })
  })

  it('Chef search does not return Scientist records', async () => {
    const res = await db.ai.search({
      propertyName: PROPERTY,
      query: 'cooking techniques and gastronomy',
      labels: [LABEL_B],
      limit: 5
    })

    expect(res.success).toBe(true)
    expect(res.data.length).toBeGreaterThan(0)

    // All results must be Chef records (bios are food-oriented)
    const bios = res.data.map((r) => String(r.bio ?? ''))
    bios.forEach((bio) => {
      // Scientist bios contain science keywords; none should appear
      expect(bio.toLowerCase()).not.toMatch(/relativity|dna|computer|quantum|electron|crystallography/)
    })
  })

  it('duplicate (label, propertyName) index creation returns 409', async () => {
    await expect(db.ai.indexes.create({ label: LABEL_A, propertyName: PROPERTY })).rejects.toMatchObject({
      status: 409
    })
  })
})
