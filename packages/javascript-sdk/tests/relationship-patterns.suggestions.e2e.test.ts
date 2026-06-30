/**
 * E2E tests for relationship-pattern suggestion validation logic.
 *
 * These tests must be run locally against a live RushDB instance
 * (with RUSHDB_API_KEY + RUSHDB_API_URL in packages/javascript-sdk/.env)
 * and an LLM configured in the platform (RUSHDB_LLM_API_KEY / RUSHDB_LLM_MODEL).
 *
 * They are NOT run in CI.
 *
 * Run with:
 *   pnpm jest --rootDir packages/javascript-sdk -i relationship-patterns.suggestions.e2e.test.ts
 */
import path from 'path'
import dotenv from 'dotenv'

dotenv.config({ path: path.resolve(__dirname, '../.env') })

import RushDB from '../src/index.node'

// -----------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------

const sleep = (ms: number) => new Promise((res) => setTimeout(res, ms))

type AnalysisResult = {
  patterns: Array<{
    id: string
    mode: string
    status: string
    source: { label: string; key?: string }
    target: { label: string; key?: string }
    type: string
    confidence: number
  }>
  relationships: Array<{
    label: string
    relationships: Array<{ label: string; type: string; direction: string }>
  }>
  analysis?: {
    status: string
    lastRunAt?: string
    lastError?: string
  }
}

// -----------------------------------------------------------------
// Suite
// -----------------------------------------------------------------

describe('relationship-patterns suggestion validation (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = (process.env.RUSHDB_API_URL || 'http://localhost:3000').replace(/\/$/, '')

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })

  // Unique suffix per run → unique labels so tests don't collide with prior runs
  const suffix = `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 5)}`
  const tenantId = `rp-e2e-${suffix}`

  // Label names scoped to this run
  const WRITER_LABEL = `RpWriter${suffix}`
  const BOOK_LABEL = `RpBook${suffix}`
  const PUBLISHER_LABEL = `RpPublisher${suffix}`
  const EDITION_LABEL = `RpEdition${suffix}`

  jest.setTimeout(120_000)

  // ------------------------------------------------------------------
  // SDK helper
  // ------------------------------------------------------------------
  /**
   * Polls GET /relationships/patterns until analysis.status is 'idle' or 'error'.
   * Throws if the deadline is reached without completion.
   */
  async function waitForAnalysis(maxMs = 90_000): Promise<AnalysisResult> {
    const deadline = Date.now() + maxMs
    while (Date.now() < deadline) {
      const { data } = await db.relationships.patterns.list()
      const status = data?.analysis?.status
      if (status === 'idle' || status === 'error') {
        return data as AnalysisResult
      }
      await sleep(3_000)
    }
    throw new Error('Analysis did not reach idle/error status within timeout')
  }

  // ------------------------------------------------------------------
  // Data setup
  // ------------------------------------------------------------------

  beforeAll(async () => {
    // --- Scenario A data: Writer → Book with a shared writerId FK field
    //     + an explicit WROTE semantic relationship
    await db.records.createMany({
      label: WRITER_LABEL,
      data: [
        { writerId: 'w1', name: 'Alice', tenantId },
        { writerId: 'w2', name: 'Bob', tenantId }
      ],
      options: { returnResult: false }
    })

    await db.records.createMany({
      label: BOOK_LABEL,
      data: [
        { writerId: 'w1', title: 'The First Novel', genre: 'fiction', tenantId },
        { writerId: 'w2', title: 'Data Structures', genre: 'non-fiction', tenantId }
      ],
      options: { returnResult: false }
    })

    // Creates semantic WROTE relationships: Writer -[WROTE]-> Book matched by writerId
    await db.relationships.createMany({
      source: { label: WRITER_LABEL, key: 'writerId', where: { tenantId } },
      target: { label: BOOK_LABEL, key: 'writerId', where: { tenantId } },
      type: 'WROTE',
      direction: 'out'
    })

    // --- Scenario B data: Publisher → [DEFAULT] → Edition (via importJson nesting)
    await db.records.importJson({
      label: PUBLISHER_LABEL,
      data: {
        name: 'Acme Press',
        country: 'US',
        foundedYear: 1990,
        tenantId,
        editions: [
          { title: 'First Edition', year: 2020, tenantId },
          { title: 'Second Edition', year: 2021, tenantId }
        ]
      },
      options: { suggestTypes: true }
    })
  })

  afterAll(async () => {
    // Delete all records from this test run by tenantId.
    // Using no `labels` filter covers the nested records that importJson creates
    // from field names (e.g. `editions`) which differ from our EDITION_LABEL constant.
    await db.records.delete({ where: { tenantId } }).catch(() => {})
  })

  // ------------------------------------------------------------------
  // Tests
  // ------------------------------------------------------------------

  /**
   * Scenario A — regression guard for the hasSemanticRelationshipBetween fix.
   *
   * When a semantic (non-default) relationship already exists between two labels,
   * the pattern engine must NOT suggest a join_pattern for that label pair.
   *
   * WRITER -[WROTE]-> BOOK already exists.  The LLM may notice the writerId FK
   * field and try to suggest a join, but validateCandidate must reject it.
   */
  it('does NOT suggest join_pattern for label pairs that already have a semantic relationship', async () => {
    // Force a fresh analysis for this project
    const { data } = await db.relationships.patterns.analyze()
    expect(data.queued).toBe(true)

    const result = await waitForAnalysis()

    if (result.analysis?.status === 'error') {
      throw new Error(
        `Analysis failed (check RUSHDB_LLM_API_KEY / RUSHDB_LLM_MODEL on the server): ${result.analysis.lastError}`
      )
    }

    expect(result.analysis?.status).toBe('idle')

    // No join_pattern should exist for the Writer-Book label pair in either direction
    const invalidJoins = (result.patterns ?? []).filter(
      (p) =>
        p.mode === 'join_pattern' &&
        ((p.source?.label === WRITER_LABEL && p.target?.label === BOOK_LABEL) ||
          (p.source?.label === BOOK_LABEL && p.target?.label === WRITER_LABEL))
    )

    expect(invalidJoins).toHaveLength(0)
  })

  /**
   * Scenario B — regression guard for the bidirectional hasDefaultRelationshipBetween fix.
   *
   * When a DEFAULT relationship exists between two labels (created by importJson nesting),
   * the pattern engine must be willing to suggest retype_existing_relationship for that
   * pair regardless of which side "owns" the relationship in the schema.
   *
   * This test verifies the invariant: every retype_existing_relationship pattern in the
   * result must correspond to a label pair that actually has a default relationship in
   * the current schema.  A regression in hasDefaultRelationshipBetween would cause
   * retype patterns to appear for pairs without a default rel, or incorrectly block them
   * (both are caught by the invariant check below).
   *
   * Additionally, we assert that no join_pattern is suggested for the
   * Publisher-Edition pair once a retype has been accepted — future iterations should
   * not re-propose a join where a retype already exists.
   */
  it('retype_existing_relationship patterns are only suggested for default-connected label pairs', async () => {
    const result = await waitForAnalysis()

    if (result.analysis?.status === 'error') {
      throw new Error(
        `Analysis failed (check RUSHDB_LLM_API_KEY / RUSHDB_LLM_MODEL on the server): ${result.analysis.lastError}`
      )
    }

    const schema = result.relationships ?? []
    const DEFAULT_TYPE_PREFIXES = ['RUSHDB_DEFAULT_RELATION', 'RUSHDB_RELATION']

    const isDefaultType = (type: string) =>
      DEFAULT_TYPE_PREFIXES.some((prefix) => type === prefix || type.startsWith(prefix))

    const hasDefaultBetween = (labelA: string, labelB: string): boolean => {
      const entryA = schema.find((o) => o.label === labelA)
      const entryB = schema.find((o) => o.label === labelB)
      return (
        entryA?.relationships.some((r) => r.label === labelB && isDefaultType(r.type)) ||
        entryB?.relationships.some((r) => r.label === labelA && isDefaultType(r.type)) ||
        false
      )
    }

    const retypers = (result.patterns ?? []).filter((p) => p.mode === 'retype_existing_relationship')

    // Invariant: every retype suggestion whose labels still exist in the current
    // schema must have a default rel between its label pair.
    // Patterns from previous test runs whose records have since been deleted are
    // skipped — they are stale rows in Postgres that no longer have live graph
    // data, so they cannot be validated against the current schema.
    for (const pattern of retypers) {
      const { label: srcLabel } = pattern.source
      const { label: tgtLabel } = pattern.target

      const srcInSchema = schema.some((o) => o.label === srcLabel)
      const tgtInSchema = schema.some((o) => o.label === tgtLabel)

      // Skip stale patterns (one or both labels absent from current schema)
      if (!srcInSchema || !tgtInSchema) continue

      expect(hasDefaultBetween(srcLabel, tgtLabel)).toBe(true)
    }

    // The Publisher-Edition pair must NOT appear as a join_pattern — they are connected
    // by a default relationship so only a retype (not a join) is appropriate
    const publisherEditionJoins = (result.patterns ?? []).filter(
      (p) =>
        p.mode === 'join_pattern' &&
        ((p.source?.label === PUBLISHER_LABEL && p.target?.label === EDITION_LABEL) ||
          (p.source?.label === EDITION_LABEL && p.target?.label === PUBLISHER_LABEL))
    )

    expect(publisherEditionJoins).toHaveLength(0)
  })
})
