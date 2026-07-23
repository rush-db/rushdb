/**
 * E2E tests for db.records.vectorSearch() with nested `where` predicates (related-record traversal).
 *
 * This file complements ai.search.e2e.test.ts which covers flat `where` filters.
 * Here we exercise the case where the `where` contains label keys that resolve to
 * OPTIONAL MATCH traversals — exactly the same syntax supported by records.find().
 *
 * HOW IT WORKS
 * ────────────
 * ai.service.ts sends the full sorted queryParts to getSemanticSearchPrefilterQuery(),
 * which emits OPTIONAL MATCH clauses before the vector-similarity scoring step.
 * Only records whose related nodes satisfy the traversal condition are included in the
 * candidate pool, and then similarity ranking is applied over that narrowed set.
 *
 * DATA MODEL
 * ──────────
 *   (ASAuthor)-[DEFAULT]→(ASPost {description, ...})
 *
 *   Alice → "Graph DB Internals"     (category=databases, published=true)
 *   Alice → "Deep Neural Networks"   (category=ml, published=true)
 *   Bob   → "French Cuisine"         (category=food, published=false)
 *
 * The embedding index is on ASPost.description.
 * Nested-where tests filter the ASPost candidate pool by properties of the
 * related (ASAuthor) node before similarity ranking.
 *
 * Prerequisites
 * ─────────────
 * RUSHDB_API_KEY and RUSHDB_API_URL must be set in packages/javascript-sdk/.env.
 * The server must be configured with RUSHDB_EMBEDDING_MODEL / RUSHDB_EMBEDDING_API_KEY.
 * Both env vars are checked; the suite is skipped gracefully if either is absent.
 */

import RushDB from '../../packages/javascript-sdk/src/index.node'
import type { EmbeddingIndex } from '../../packages/javascript-sdk/src/api/types'

jest.setTimeout(240_000) // backfill cron ticks once a minute; a slow provider can add most of another

// ─────────────────────────────────────────────────────────────────────────────
// Helper
// ─────────────────────────────────────────────────────────────────────────────

async function waitForIndexReady(
  db: RushDB,
  indexId: string,
  timeoutMs = 180_000,
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
// Guard: skip when credentials or embedding config are absent
// ─────────────────────────────────────────────────────────────────────────────

const apiKey = process.env.RUSHDB_API_KEY
const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

if (!apiKey || process.env.RUSHDB_E2E_EMBEDDINGS === 'false') {
  describe('records.vectorSearch() – nested where prefilter (e2e)', () => {
    it('skips: no RUSHDB_API_KEY or server-side embeddings unavailable', () => expect(true).toBe(true))
  })
} else {
  const db = new RushDB(apiKey, { url: apiUrl })

  const tenantId = `ai-nested-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  const L_POST = 'ASPost'
  const L_AUTHOR = 'ASAuthor'
  const PROPERTY = 'description'

  let indexId: string | undefined

  // ── Test dataset ───────────────────────────────────────────────────────────
  // Posts; each nested array creates an ASAuthor linked to the post via importJson

  const postsWithAuthors = [
    {
      title: 'Graph DB Internals',
      description:
        'Graph databases store data as nodes and edges, enabling efficient traversal of connected data structures.',
      category: 'databases',
      published: true,
      tenantId,
      [L_AUTHOR]: [{ name: 'Alice', seniority: 10, tenantId }]
    },
    {
      title: 'Deep Neural Networks',
      description:
        'Deep neural networks consist of multiple hidden layers that progressively extract higher-level features from raw input.',
      category: 'ml',
      published: true,
      tenantId,
      [L_AUTHOR]: [{ name: 'Alice', seniority: 10, tenantId }]
    },
    {
      title: 'French Cuisine',
      description:
        'French cuisine is celebrated worldwide for its refined techniques, rich sauces, and fresh ingredients.',
      category: 'food',
      published: false,
      tenantId,
      [L_AUTHOR]: [{ name: 'Bob', seniority: 2, tenantId }]
    }
  ]

  // ── Setup ──────────────────────────────────────────────────────────────────
  beforeAll(async () => {
    // Clean up any overlap from a previous aborted run
    await db.records.delete({ where: { tenantId } }).catch(() => {})

    // Insert posts + authors via importJson (builds ASPost→ASAuthor graph)
    await db.records.importJson({ label: L_POST, data: postsWithAuthors })

    // Create embedding index on ASPost.description
    let createdIndex: EmbeddingIndex
    try {
      const res = await db.ai.indexes.create({ label: L_POST, propertyName: PROPERTY })
      createdIndex = res.data as EmbeddingIndex
    } catch (err: any) {
      // 409 = index already exists from a previous run — reuse it
      const list = await db.ai.indexes.find()
      const existing = (list.data as EmbeddingIndex[]).find(
        (i) => i.propertyName === PROPERTY && i.label === L_POST
      )
      if (!existing) throw err
      createdIndex = existing
    }

    indexId = createdIndex.id
    await waitForIndexReady(db, indexId)
  })

  // ── Teardown ───────────────────────────────────────────────────────────────
  afterAll(async () => {
    await db.records.delete({ where: { tenantId } }).catch(() => {})
    if (indexId) await db.ai.indexes.delete(indexId).catch(() => {})
  })

  describe('records.vectorSearch() – nested where prefilter (e2e)', () => {
    // ══════════════════════════════════════════════════════════════════════
    // 1. Flat where (baseline — verifies no regression in simple case)
    // ══════════════════════════════════════════════════════════════════════
    describe('baseline: flat where still works', () => {
      it('restricts candidates to published posts', async () => {
        const res = await db.records.vectorSearch({
          propertyName: PROPERTY,
          query: 'cooking recipes and culinary arts',
          labels: [L_POST],
          where: { published: false, tenantId },
          limit: 5
        })

        // Only "French Cuisine" is unpublished; the query targets it semantically
        const titles = res.data.map((r) => r.data.title)
        expect(titles).toContain('French Cuisine')
        // Published posts must not appear when we narrow to published=false
        expect(titles).not.toContain('Graph DB Internals')
        expect(titles).not.toContain('Deep Neural Networks')
      })
    })

    // ══════════════════════════════════════════════════════════════════════
    // 2. Nested where — filter by related author name
    // ══════════════════════════════════════════════════════════════════════
    describe('nested where: filter by related author property', () => {
      it('returns only posts whose ASAuthor is Alice', async () => {
        const res = await db.records.vectorSearch({
          propertyName: PROPERTY,
          query: 'data structures and algorithms',
          labels: [L_POST],
          where: {
            tenantId,
            [L_AUTHOR]: {
              name: 'Alice'
            }
          },
          limit: 10
        })

        expect(res.data.length).toBeGreaterThan(0)

        const titles = res.data.map((r) => r.data.title)
        // Both Alice's posts are eligible candidates
        expect(titles).toContain('Graph DB Internals')
        expect(titles).toContain('Deep Neural Networks')
        // Bob's post must not appear
        expect(titles).not.toContain('French Cuisine')
      })

      it('returns only posts whose ASAuthor is Bob', async () => {
        const res = await db.records.vectorSearch({
          propertyName: PROPERTY,
          query: 'cooking and gastronomy',
          labels: [L_POST],
          where: {
            tenantId,
            [L_AUTHOR]: {
              name: 'Bob'
            }
          },
          limit: 10
        })

        expect(res.data.length).toBeGreaterThan(0)

        const titles = res.data.map((r) => r.data.title)
        expect(titles).toContain('French Cuisine')
        expect(titles).not.toContain('Graph DB Internals')
        expect(titles).not.toContain('Deep Neural Networks')
      })
    })

    // ══════════════════════════════════════════════════════════════════════
    // 3. Nested where — range predicate on related record
    // ══════════════════════════════════════════════════════════════════════
    describe('nested where: range predicate on related record', () => {
      it('returns only posts by senior authors (seniority >= 8)', async () => {
        const res = await db.records.vectorSearch({
          propertyName: PROPERTY,
          query: 'machine learning neural networks',
          labels: [L_POST],
          where: {
            tenantId,
            [L_AUTHOR]: {
              seniority: { $gte: 8 }
            }
          },
          limit: 10
        })

        expect(res.data.length).toBeGreaterThan(0)

        const titles = res.data.map((r) => r.data.title)
        // Alice (seniority=10) authored both technical posts
        expect(titles).toContain('Deep Neural Networks')
        expect(titles).toContain('Graph DB Internals')
        // Bob (seniority=2) wrote French Cuisine — must be excluded
        expect(titles).not.toContain('French Cuisine')
      })
    })

    // ══════════════════════════════════════════════════════════════════════
    // 4. Mixed: root-record predicate AND nested author predicate
    // ══════════════════════════════════════════════════════════════════════
    describe('mixed: root predicate + nested author predicate', () => {
      it('combines published=true with author name filter', async () => {
        const res = await db.records.vectorSearch({
          propertyName: PROPERTY,
          query: 'graph nodes edges connected',
          labels: [L_POST],
          where: {
            tenantId,
            published: true,
            [L_AUTHOR]: {
              name: 'Alice'
            }
          },
          limit: 10
        })

        const titles = res.data.map((r) => r.data.title)
        // Alice's published posts: both tech posts
        expect(titles).toContain('Graph DB Internals')
        expect(titles).toContain('Deep Neural Networks')
        // French Cuisine: Bob's AND unpublished — excluded by both conditions
        expect(titles).not.toContain('French Cuisine')
      })

      it('returns empty when root predicate conflicts with nested predicate', async () => {
        // published=false AND ASAuthor.name="Alice"  — Alice has no drafted posts
        const res = await db.records.vectorSearch({
          propertyName: PROPERTY,
          query: 'deep learning',
          labels: [L_POST],
          where: {
            tenantId,
            published: false,
            [L_AUTHOR]: {
              name: 'Alice'
            }
          },
          limit: 10
        })

        // No post satisfies both conditions simultaneously
        expect(res.data.length).toBe(0)
      })
    })

    // ══════════════════════════════════════════════════════════════════════
    // 5. Results are still ranked by similarity score (descending)
    // ══════════════════════════════════════════════════════════════════════
    describe('similarity ranking preserved after nested prefilter', () => {
      it('returns results in descending __score order when nested filter applies', async () => {
        const res = await db.records.vectorSearch({
          propertyName: PROPERTY,
          query: 'data science and machine learning',
          labels: [L_POST],
          where: {
            tenantId,
            [L_AUTHOR]: { name: 'Alice' }
          },
          limit: 10
        })

        expect(res.data.length).toBeGreaterThan(1)

        // Every result must have a numeric score
        const scores = res.data.map((r) => r.data.__score)
        scores.forEach((s) => {
          expect(typeof s).toBe('number')
          expect(s).toBeGreaterThan(0)
          expect(s).toBeLessThanOrEqual(1)
        })

        // Scores must be non-increasing
        for (let i = 0; i < scores.length - 1; i++) {
          expect(scores[i]).toBeGreaterThanOrEqual(scores[i + 1])
        }
      })
    })
  })
}
