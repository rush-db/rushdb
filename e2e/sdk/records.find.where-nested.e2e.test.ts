/**
 * E2E tests for complex `where` predicates that traverse related records.
 *
 * The RushDB SearchQuery parser interprets any nested object key that is not
 * an operator prefix as a label for a related record traversal. This test
 * file exercises every documented variant:
 *
 *   1. Simple related-record filter  — WHERE RelatedLabel.prop = value
 *   2. Compound predicates on related record — $and / $or on related fields
 *   3. Relationship direction — $relation: { direction }
 *   4. Multi-level traversal — A → B → C
 *   5. Mixed: root-record predicate AND related-record predicate
 *   6. $id filter on related record
 *   7. Negative / empty result cases
 *
 * Data model (created via importJson — one call builds the whole graph):
 *
 *   (WNAuthor)-[DEFAULT]→(WNPost)-[DEFAULT]→(WNTag)
 *
 *   Alice (seniority=10)
 *     └─→ "Graph Databases" (published, views=1200)
 *           └─→ tag "graphs"
 *           └─→ tag "databases"
 *     └─→ "Draft Post" (draft, views=0)
 *   Bob (seniority=3)
 *     └─→ "Machine Learning" (published, views=500)
 *           └─→ tag "ml"
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

jest.setTimeout(60_000)

const apiKey = process.env.RUSHDB_API_KEY
const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

if (!apiKey) {
  describe('records.find() – nested where predicates (e2e)', () => {
    it('skips because RUSHDB_API_KEY is not set', () => expect(true).toBe(true))
  })
} else {
  const db = new RushDB(apiKey, { url: apiUrl })

  // Unique tenant so parallel runs never collide
  const tenantId = `where-nested-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

  // ── Labels ────────────────────────────────────────────────────────────────
  const L_AUTHOR = 'WNAuthor'
  const L_POST = 'WNPost'
  const L_TAG = 'WNTag'

  // ── IDs resolved after import ─────────────────────────────────────────────
  let authorAliceId: string
  let tagGraphsId: string

  // ── Setup ─────────────────────────────────────────────────────────────────
  beforeAll(async () => {
    // Purge any leftover data from previous interrupted runs
    await db.records.delete({ where: { tenantId } }).catch(() => {})

    // Build the whole Author → Post → Tag graph with a single importJson call.
    // importJson creates nested records and links each child to its parent via
    // the platform's default relationship.
    await db.records.importJson({
      label: L_AUTHOR,
      data: [
        {
          name: 'Alice',
          seniority: 10,
          tenantId,
          [L_POST]: [
            {
              title: 'Graph Databases',
              status: 'published',
              views: 1200,
              tenantId,
              [L_TAG]: [
                { name: 'graphs', tenantId },
                { name: 'databases', tenantId }
              ]
            },
            {
              title: 'Draft Post',
              status: 'draft',
              views: 0,
              tenantId
            }
          ]
        },
        {
          name: 'Bob',
          seniority: 3,
          tenantId,
          [L_POST]: [
            {
              title: 'Machine Learning',
              status: 'published',
              views: 500,
              tenantId,
              [L_TAG]: [{ name: 'ml', tenantId }]
            }
          ]
        }
      ]
    })

    // importJson returns top-level records but record property access may differ;
    // look up IDs explicitly via find to be safe.
    const aliceRes = await db.records.find({ labels: [L_AUTHOR], where: { tenantId, name: 'Alice' } })
    authorAliceId = (aliceRes.data as any[])[0]?.id

    // Fetch the "graphs" tag ID for the 2-hop $id test.
    const tagsRes = await db.records.find({ labels: [L_TAG], where: { tenantId, name: 'graphs' } })
    tagGraphsId = (tagsRes.data as any[])[0]?.id
  })

  // ── Teardown ──────────────────────────────────────────────────────────────
  afterAll(async () => {
    await db.records.delete({ where: { tenantId } }).catch(() => {})
  })

  describe('records.find() – nested where predicates (e2e)', () => {
    // ══════════════════════════════════════════════════════════════════════
    // 1. Simple related-record filter
    // ══════════════════════════════════════════════════════════════════════
    describe('simple related-record filter', () => {
      it('finds authors connected to a post with a specific title', async () => {
        const res = await db.records.find({
          labels: [L_AUTHOR],
          where: {
            tenantId,
            [L_POST]: {
              title: 'Graph Databases'
            }
          }
        })
        const names = (res.data as any[]).map((r) => r.data.name)
        expect(names).toContain('Alice')
        expect(names).not.toContain('Bob')
      })

      it('finds posts connected to a tag named "graphs"', async () => {
        const res = await db.records.find({
          labels: [L_POST],
          where: {
            tenantId,
            [L_TAG]: {
              name: 'graphs'
            }
          }
        })
        const titles = (res.data as any[]).map((r) => r.data.title)
        expect(titles).toContain('Graph Databases')
        expect(titles).not.toContain('Machine Learning')
        expect(titles).not.toContain('Draft Post')
      })
    })

    // ══════════════════════════════════════════════════════════════════════
    // 2. Compound predicates on related record
    // ══════════════════════════════════════════════════════════════════════
    describe('compound predicates on related record', () => {
      it('finds posts where related tag name is one of multiple values ($or)', async () => {
        const res = await db.records.find({
          labels: [L_POST],
          where: {
            tenantId,
            [L_TAG]: {
              $or: [{ name: 'graphs' }, { name: 'ml' }]
            }
          }
        })
        const titles = (res.data as any[]).map((r) => r.data.title)
        expect(titles).toContain('Graph Databases')
        expect(titles).toContain('Machine Learning')
        expect(titles).not.toContain('Draft Post')
      })

      it('finds authors connected to a published post with views >= 1000 ($and on related)', async () => {
        const res = await db.records.find({
          labels: [L_AUTHOR],
          where: {
            tenantId,
            [L_POST]: {
              $and: [{ status: 'published' }, { views: { $gte: 1000 } }]
            }
          }
        })
        const names = (res.data as any[]).map((r) => r.data.name)
        // Only Alice wrote the high-view published post (1200 views)
        expect(names).toContain('Alice')
        expect(names).not.toContain('Bob') // Bob's post has only 500 views
      })
    })

    // ══════════════════════════════════════════════════════════════════════
    // 3. Relationship direction
    // ══════════════════════════════════════════════════════════════════════
    describe('relationship direction', () => {
      it('filters by incoming relationship direction (direction: in)', async () => {
        // Query Posts; traverse inbound relationships to WNAuthor nodes.
        // Because importJson creates Author → Post edges (parent-to-child = outbound),
        // from the Post's perspective the connection to the Author is "in".
        const res = await db.records.find({
          labels: [L_POST],
          where: {
            tenantId,
            [L_AUTHOR]: {
              $relation: { direction: 'in' },
              seniority: { $gte: 8 }
            }
          }
        })
        const titles = (res.data as any[]).map((r) => r.data.title)
        // Alice (seniority=10) authored "Graph Databases" and "Draft Post"
        expect(titles).toContain('Graph Databases')
        expect(titles).toContain('Draft Post')
        // Bob (seniority=3) authored "Machine Learning" — should be excluded
        expect(titles).not.toContain('Machine Learning')
      })

      it('filters by outgoing relationship direction (direction: out)', async () => {
        // Query Authors; traverse outbound relationships to WNPost nodes.
        const res = await db.records.find({
          labels: [L_AUTHOR],
          where: {
            tenantId,
            [L_POST]: {
              $relation: { direction: 'out' },
              status: 'draft'
            }
          }
        })
        const names = (res.data as any[]).map((r) => r.data.name)
        // Only Alice has a draft post
        expect(names).toContain('Alice')
        expect(names).not.toContain('Bob')
      })
    })

    // ══════════════════════════════════════════════════════════════════════
    // 4. Multi-level traversal (Author → Post → Tag)
    // ══════════════════════════════════════════════════════════════════════
    describe('multi-level traversal', () => {
      it('finds authors whose posts are tagged with "graphs" (2-hop)', async () => {
        const res = await db.records.find({
          labels: [L_AUTHOR],
          where: {
            tenantId,
            [L_POST]: {
              [L_TAG]: {
                name: 'graphs'
              }
            }
          }
        })
        const names = (res.data as any[]).map((r) => r.data.name)
        expect(names).toContain('Alice')
        expect(names).not.toContain('Bob')
      })

      it('finds authors whose posts are tagged with "ml" (2-hop)', async () => {
        const res = await db.records.find({
          labels: [L_AUTHOR],
          where: {
            tenantId,
            [L_POST]: {
              [L_TAG]: {
                name: 'ml'
              }
            }
          }
        })
        const names = (res.data as any[]).map((r) => r.data.name)
        expect(names).toContain('Bob')
        expect(names).not.toContain('Alice')
      })
    })

    // ══════════════════════════════════════════════════════════════════════
    // 5. Mixed: root-record predicate AND related-record predicate
    // ══════════════════════════════════════════════════════════════════════
    describe('mixed root + related predicates', () => {
      it('finds senior authors (seniority > 5) who also have a published post', async () => {
        const res = await db.records.find({
          labels: [L_AUTHOR],
          where: {
            tenantId,
            seniority: { $gt: 5 },
            [L_POST]: {
              status: 'published'
            }
          }
        })
        const names = (res.data as any[]).map((r) => r.data.name)
        expect(names).toContain('Alice') // seniority=10, has published post
        expect(names).not.toContain('Bob') // seniority=3, below threshold
      })

      it('finds published posts with high views that also have a tag', async () => {
        const res = await db.records.find({
          labels: [L_POST],
          where: {
            tenantId,
            status: 'published',
            views: { $gte: 500 },
            [L_TAG]: {
              tenantId
            }
          }
        })
        const titles = (res.data as any[]).map((r) => r.data.title)
        expect(titles).toContain('Graph Databases') // published, 1200 views, has tags
        expect(titles).toContain('Machine Learning') // published, 500 views, has tags
        expect(titles).not.toContain('Draft Post') // status=draft, no tags
      })
    })

    // ══════════════════════════════════════════════════════════════════════
    // 6. $id filter on related record
    // ══════════════════════════════════════════════════════════════════════
    describe('$id filter on related record', () => {
      it('finds posts connected to a specific author by $id', async () => {
        const res = await db.records.find({
          labels: [L_POST],
          where: {
            tenantId,
            [L_AUTHOR]: {
              $id: authorAliceId
            }
          }
        })
        const titles = (res.data as any[]).map((r) => r.data.title)
        expect(titles).toContain('Graph Databases')
        expect(titles).toContain('Draft Post')
        expect(titles).not.toContain('Machine Learning')
      })

      it('finds authors connected (via post) to a specific tag $id (2-hop $id)', async () => {
        const res = await db.records.find({
          labels: [L_AUTHOR],
          where: {
            tenantId,
            [L_POST]: {
              [L_TAG]: {
                $id: tagGraphsId
              }
            }
          }
        })
        const names = (res.data as any[]).map((r) => r.data.name)
        expect(names).toContain('Alice')
        expect(names).not.toContain('Bob')
      })
    })

    // ══════════════════════════════════════════════════════════════════════
    // 7. Negative / empty result cases
    // ══════════════════════════════════════════════════════════════════════
    describe('negative cases', () => {
      it('returns empty when no related record matches the nested predicate', async () => {
        const res = await db.records.find({
          labels: [L_AUTHOR],
          where: {
            tenantId,
            [L_POST]: {
              title: 'Does Not Exist'
            }
          }
        })
        expect(res.total).toBe(0)
        expect(res.data).toHaveLength(0)
      })

      it('returns empty when root predicate fails even though related predicate would match', async () => {
        const res = await db.records.find({
          labels: [L_AUTHOR],
          where: {
            tenantId,
            seniority: { $gt: 999 }, // no author has seniority > 999
            [L_POST]: { status: 'published' }
          }
        })
        expect(res.total).toBe(0)
      })
    })
  })
}
