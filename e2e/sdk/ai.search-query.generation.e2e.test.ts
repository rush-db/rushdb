/**
 * E2E tests for the AI SearchQuery generation endpoint (`POST /api/v1/ai/search-query`)
 * and for the engine semantics its validator exists to guard.
 *
 * WHY THIS SUITE EXISTS
 * ─────────────────────
 * SmartSearch regressions are prompt regressions: the LLM prompts
 * (platform/core/src/core/ai/search-query-{builder,spec}.prompt.md) encode query-shape
 * rules through worked examples, and models generalize from those examples' surface
 * tokens more than from their prose. A prompt that was only ever exercised against the
 * dataset its examples were written from cannot show that gap. This suite therefore
 * seeds a deliberately HOSTILE dataset whose naming avoids every token used in the
 * prompts and in typical demo data:
 *
 *   - display properties are `title` / `fullName` — NOT `name`
 *   - the scalar reference is `winningDriverRef` — camelCase, no `_id` suffix
 *   - labels are MixedCase (`RaceEvent`) — not UPPER_CASE like the prompt examples
 *
 * Dataset (all records carry `seedMark` for cross-run cleanup):
 *
 *   RaceDriver (4): Ayla, Bruno, Kenji, Marta — display `fullName`, join key `driverCode`
 *   RaceEvent  (6): display `title`; scalar `winningDriverRef` records who WON
 *                   (wins: Ayla 3, Bruno 2, Kenji 1 — no "won" relationship exists!)
 *                   PARTICIPATED_IN edges record who RACED
 *                   (participation: Bruno 5, Ayla 4, Kenji 2, Marta 1)
 *     -> "who won the most" and "who participated in the most" have DIFFERENT answers,
 *        so a semantically wrong query shape produces a visibly wrong result.
 *   SupplyDepot (5): SHIPS_TO ring North -> South -> East -> North, plus a linear
 *                    chain West -> Hub (cycle tests must return only the ring).
 *
 * TWO TEST LAYERS
 * ───────────────
 * 1. Engine guardrails (always run): pin the /records/search semantics the generator's
 *    validator protects against — correlated `where` references and `$ref`-in-where
 *    match NOTHING silently (they are matched as literal strings), which is exactly
 *    why the generator must reject them instead of returning a "valid" empty query.
 * 2. LLM generation (gated): requires the server to run with RUSHDB_LLM_API_KEY and
 *    RUSHDB_LLM_MODEL (export them before `pnpm test:e2e`; global-setup passes host
 *    env through to platform/core). Skipped gracefully when the endpoint returns 503.
 *    These are eval-style tests: they assert structural invariants of the generated
 *    queries and EXECUTE them against the seeded data. A failure here means a prompt
 *    or validator change regressed a scenario, not that the code is broken.
 *
 * Prerequisites: RUSHDB_API_KEY / RUSHDB_API_URL (harness-provisioned or from
 * packages/javascript-sdk/.env). Whole suite skips without an API key.
 */

import RushDB from '../../packages/javascript-sdk/src/index.node'

jest.setTimeout(180_000)

const apiKey = process.env.RUSHDB_API_KEY
const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'
const apiBase = `${apiUrl.replace(/\/$/, '')}/api/v1`

if (!apiKey) {
  describe('ai.search-query generation (e2e)', () => {
    it('skips because RUSHDB_API_KEY is not set', () => expect(true).toBe(true))
  })
} else {
  const db = new RushDB(apiKey, { url: apiUrl })

  const seedMark = 'ai-sqgen-suite'
  const L_DRIVER = 'RaceDriver'
  const L_EVENT = 'RaceEvent'
  const L_DEPOT = 'SupplyDepot'

  const WINS = { driver_ayla: 3, driver_bruno: 2, driver_kenji: 1 } as Record<string, number>
  const PARTICIPATION = { driver_bruno: 5, driver_ayla: 4, driver_kenji: 2, driver_marta: 1 }
  const CYCLE_MEMBER_TITLES = ['Depot East', 'Depot North', 'Depot South']

  // ── Raw HTTP helpers (endpoints the SDK does not expose) ───────────────────
  const post = async (path: string, body: unknown): Promise<{ status: number; json: any }> => {
    const res = await fetch(`${apiBase}${path}`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${apiKey}`, 'Content-Type': 'application/json' },
      body: JSON.stringify(body)
    })
    return { status: res.status, json: await res.json().catch(() => ({})) }
  }

  const searchRecords = (searchQuery: unknown) => post('/records/search', searchQuery)

  /**
   * Calls the generation endpoint. Retries only transient LLM transport failures
   * ("AI returned invalid JSON." / network hiccups) — a "Generated query is not valid"
   * 400 is a real prompt/validator regression and must surface as a test failure.
   * Success responses are wrapped by the platform as { data: { searchQuery, warnings } }.
   */
  const generate = async (
    prompt: string,
    retries = 2
  ): Promise<{ status: number; json: any; searchQuery: any }> => {
    for (let attempt = 0; ; attempt++) {
      const result = await post('/ai/search-query', { prompt })
      const transient =
        result.status >= 500 ||
        (result.status === 400 && String(result.json?.message ?? '').includes('AI returned invalid JSON'))
      if (!transient || attempt >= retries) {
        const payload = result.json?.data ?? result.json
        return { ...result, searchQuery: payload?.searchQuery }
      }
    }
  }

  /** No correlated references anywhere inside where — the invariant this suite guards. */
  const expectNoCorrelatedRefs = (where: unknown) => {
    const text = JSON.stringify(where ?? {})
    expect(text).not.toContain('"$ref"')
    expect(text).not.toContain('$record.')
  }

  /** Finds the traversal/cycle blocks of a where object as [key, value] entries. */
  const whereEntries = (where: unknown): Array<[string, any]> =>
    where && typeof where === 'object' ? Object.entries(where as Record<string, any>) : []

  // ── Seed ────────────────────────────────────────────────────────────────────
  beforeAll(async () => {
    // Cross-run cleanup: seedMark is constant, so leftovers from any previous run go too.
    await db.records.delete({ where: { seedMark } }).catch(() => {})

    await db.records.createMany({
      label: L_DRIVER,
      data: [
        { fullName: 'Ayla Ferreira', driverCode: 'driver_ayla', careerPoints: 480, seedMark },
        { fullName: 'Bruno Costa', driverCode: 'driver_bruno', careerPoints: 310, seedMark },
        { fullName: 'Kenji Watanabe', driverCode: 'driver_kenji', careerPoints: 150, seedMark },
        { fullName: 'Marta Vidal', driverCode: 'driver_marta', careerPoints: 12, seedMark }
      ],
      options: { returnResult: false }
    })

    // participantA..C are join columns for edge creation only; winningDriverRef is the
    // scalar condition property that intentionally has NO relationship counterpart.
    await db.records.createMany({
      label: L_EVENT,
      data: [
        // prettier-ignore
        { title: 'Monaco Grand Prix', seasonYear: 2023, winningDriverRef: 'driver_ayla', participantA: 'driver_ayla', participantB: 'driver_bruno', seedMark },
        // prettier-ignore
        { title: 'Silverstone Sprint', seasonYear: 2023, winningDriverRef: 'driver_ayla', participantA: 'driver_ayla', participantB: 'driver_bruno', participantC: 'driver_kenji', seedMark },
        // prettier-ignore
        { title: 'Suzuka Endurance', seasonYear: 2024, winningDriverRef: 'driver_bruno', participantA: 'driver_ayla', participantB: 'driver_bruno', seedMark },
        // prettier-ignore
        { title: 'Monza Classic', seasonYear: 2024, winningDriverRef: 'driver_bruno', participantA: 'driver_bruno', participantB: 'driver_marta', seedMark },
        // prettier-ignore
        { title: 'Spa Rain Cup', seasonYear: 2025, winningDriverRef: 'driver_ayla', participantA: 'driver_ayla', participantB: 'driver_bruno', seedMark },
        // prettier-ignore
        { title: 'Desert Night 500', seasonYear: 2025, winningDriverRef: 'driver_kenji', participantA: 'driver_kenji', seedMark }
      ],
      options: { returnResult: false }
    })

    for (const key of ['participantA', 'participantB', 'participantC']) {
      await db.relationships.createMany({
        source: { label: L_EVENT, key, where: { seedMark } },
        target: { label: L_DRIVER, key: 'driverCode', where: { seedMark } },
        type: 'PARTICIPATED_IN',
        direction: 'in'
      })
    }

    // Ring North -> South -> East -> North; linear West -> Hub.
    await db.records.createMany({
      label: L_DEPOT,
      data: [
        { title: 'Depot North', regionCode: 'N', shipsToTitle: 'Depot South', seedMark },
        { title: 'Depot South', regionCode: 'S', shipsToTitle: 'Depot East', seedMark },
        { title: 'Depot East', regionCode: 'E', shipsToTitle: 'Depot North', seedMark },
        { title: 'Depot West', regionCode: 'W', shipsToTitle: 'Depot Hub', seedMark },
        { title: 'Depot Hub', regionCode: 'H', seedMark }
      ],
      options: { returnResult: false }
    })
    await db.relationships.createMany({
      source: { label: L_DEPOT, key: 'shipsToTitle', where: { seedMark } },
      target: { label: L_DEPOT, key: 'title', where: { seedMark } },
      type: 'SHIPS_TO',
      direction: 'out'
    })
  })

  afterAll(async () => {
    await db.records.delete({ where: { seedMark } }).catch(() => {})
  })

  // ── 1. Engine guardrails — the semantics the generator validator protects ──
  describe('engine guardrails on hostile-named data (no LLM required)', () => {
    it('grouped ranking over a scalar reference property executes and orders correctly', async () => {
      const { status, json } = await searchRecords({
        labels: [L_EVENT],
        where: { seedMark },
        select: { driver: '$record.winningDriverRef', wins: { $count: '*' } },
        groupBy: ['$record.winningDriverRef'],
        orderBy: { wins: 'desc' }
      })

      expect(status).toBe(200)
      const rows = json.data as Array<{ winningDriverRef: string; wins: number }>
      expect(rows[0]).toMatchObject({ winningDriverRef: 'driver_ayla', wins: WINS.driver_ayla })
      const byDriver = Object.fromEntries(rows.map((row) => [row.winningDriverRef, row.wins]))
      expect(byDriver).toMatchObject(WINS)
    })

    it('a correlated where value ("$record.id") is matched as a literal and returns zero rows', async () => {
      const { status, json } = await searchRecords({
        labels: [L_DRIVER],
        where: { seedMark, [L_EVENT]: { winningDriverRef: '$record.id' } },
        limit: 10
      })

      expect(status).toBe(200)
      expect(json.total).toBe(0)
    })

    it('"$ref" inside where matches nothing silently — why the generator must reject it upstream', async () => {
      const { status, json } = await searchRecords({
        labels: [L_EVENT],
        where: { seedMark, winningDriverRef: { $ref: '$record.id' } },
        limit: 10
      })

      expect(status).toBe(200)
      expect(json.total).toBe(0)
    })

    it('a $cycle query returns exactly the ring members, not the linear chain', async () => {
      const { status, json } = await searchRecords({
        labels: [L_DEPOT],
        where: {
          seedMark,
          $cycle: { type: 'SHIPS_TO', direction: 'out', hops: { min: 2, max: 6 } }
        }
      })

      expect(status).toBe(200)
      const titles = (json.data as any[]).map((record) => record.title).sort()
      expect(titles).toEqual(CYCLE_MEMBER_TITLES)
    })

    it('a bounded hops traversal reaches multi-hop neighbours', async () => {
      // Two directed hops from Depot North: South (1) and East (2). West/Hub unreachable.
      const { status, json } = await searchRecords({
        labels: [L_DEPOT],
        where: {
          seedMark,
          [L_DEPOT]: {
            $relation: { type: 'SHIPS_TO', direction: 'in', hops: { min: 1, max: 2 } },
            title: 'Depot North'
          }
        }
      })

      expect(status).toBe(200)
      const titles = (json.data as any[]).map((record) => record.title).sort()
      expect(titles).toEqual(['Depot East', 'Depot South'])
    })
  })

  // ── 2. LLM generation — gated on the server having an LLM configured ───────
  describe('LLM generation against hostile naming (requires RUSHDB_LLM_* on the server)', () => {
    let aiEnabled = false

    beforeAll(async () => {
      // Make the freshly seeded labels visible to the generator (its schema is cached).
      const schema = await post('/ai/schema/md', { force: true })
      expect(schema.status).toBe(200)
      expect(String(schema.json?.data ?? '')).toContain(L_EVENT)

      const probe = await generate('List all records')
      aiEnabled = probe.status !== 503
      if (!aiEnabled) {
        console.warn('[ai.search-query e2e] RUSHDB_LLM_* not configured on server — skipping LLM tests')
      }
    })

    const llmIt = (title: string, fn: () => Promise<void>) =>
      it(title, async () => {
        if (!aiEnabled) return
        await fn()
      })

    llmIt('condition recorded only as a scalar property: roots on the owning label and groups by it', async () => {
      const { status, searchQuery: query } = await generate('Which driver won the most races?')

      expect(status).toBe(200)
      expectNoCorrelatedRefs(query.where)
      // "won" exists only as RaceEvent.winningDriverRef — the exception shape is the
      // ONLY valid query. Rooting on RaceDriver would count participation, not wins.
      expect(query.labels).toEqual([L_EVENT])
      expect(JSON.stringify(query.groupBy)).toContain('winningDriverRef')

      const executed = await searchRecords(query)
      expect(executed.status).toBe(200)
      const top = (executed.json.data as any[])[0]
      expect(top.winningDriverRef).toBe('driver_ayla')
      expect(Object.values(top)).toContain(WINS.driver_ayla)
    })

    llmIt('condition backed by a real relationship: keeps the asked-for entity as root and traverses', async () => {
      const { status, searchQuery: query } = await generate('Which driver participated in the most races?')

      expect(status).toBe(200)
      expectNoCorrelatedRefs(query.where)
      expect(query.labels).toEqual([L_DRIVER])
      // Must traverse RaceEvent (participation IS a relationship) and rank by count.
      const traversal = whereEntries(query.where).find(([key]) => key === L_EVENT)
      expect(traversal).toBeDefined()
      expect(JSON.stringify(query.select)).toContain('$count')

      const executed = await searchRecords(query)
      expect(executed.status).toBe(200)
      const top = (executed.json.data as any[])[0]
      // Bruno leads participation (5) while Ayla leads wins — a wins-shaped query here
      // would surface Ayla and fail this assertion.
      expect(JSON.stringify(top)).toContain('Bruno')
      expect(Object.values(top)).toContain(PARTICIPATION.driver_bruno)
    })

    llmIt('cycle intent produces a valid $cycle operator that finds exactly the ring', async () => {
      const { status, searchQuery: query } = await generate('Which supply depots ship to each other in a circular loop?')

      expect(status).toBe(200)
      expectNoCorrelatedRefs(query.where)
      expect(query.labels).toEqual([L_DEPOT])
      // The spec prompt teaches the operator form: $cycle's value IS the traversal spec.
      const cycleOperator = whereEntries(query.where).find(([key]) => key === '$cycle')
      expect(cycleOperator).toBeDefined()
      expect(cycleOperator![1]?.hops).toBeDefined()

      const executed = await searchRecords(query)
      expect(executed.status).toBe(200)
      const titles = (executed.json.data as any[])
        .filter((record) => record.seedMark === seedMark)
        .map((record) => record.title)
        .sort()
      expect(titles).toEqual(CYCLE_MEMBER_TITLES)
    })

    llmIt('hops-away intent roots on returned records, not the named origin', async () => {
      const { status, searchQuery: query } = await generate('Find supply depots that are 1-2 hops away from Depot North')

      expect(status).toBe(200)
      expectNoCorrelatedRefs(query.where)
      expect(query.labels).toEqual([L_DEPOT])

      const traversal = whereEntries(query.where).find(([key]) => key === L_DEPOT)
      expect(traversal).toBeDefined()
      expect(JSON.stringify(traversal![1])).toContain('Depot North')
      expect(traversal![1]?.$relation?.direction).toBe('in')
      expect(traversal![1]?.$relation?.hops).toMatchObject({ min: 1, max: 2 })

      const executed = await searchRecords(query)
      expect(executed.status).toBe(200)
      expect(executed.json.total).not.toBe(1)
      const rows = (executed.json.data as any[]).filter((record) => JSON.stringify(record).includes(seedMark))
      const titles = rows.map((record) => record.title).sort()
      expect(titles).toEqual(['Depot East', 'Depot South'])
    })

    llmIt('display-property filters land on the schema-listed property, not an assumed "name"', async () => {
      const { status, searchQuery: query } = await generate('Find the Monaco race')

      expect(status).toBe(200)
      expectNoCorrelatedRefs(query.where)
      expect(query.labels).toEqual([L_EVENT])
      const whereText = JSON.stringify(query.where)
      expect(whereText).toContain('"title"')
      expect(whereText).not.toContain('"name"')

      const executed = await searchRecords(query)
      expect(executed.status).toBe(200)
      const titles = (executed.json.data as any[]).map((record) => record.title)
      expect(titles).toContain('Monaco Grand Prix')
    })

    llmIt('SDK db.ai.search(prompt) generates and executes a SearchQuery', async () => {
      const result = await db.ai.search('Find the Monaco race')

      expect(result.searchQuery).toBeDefined()
      expect(Array.isArray(result.warnings)).toBe(true)
      expect(result.data.length).toBeGreaterThan(0)
      const titles = result.data.map((record) => record.data.title)
      expect(titles).toContain('Monaco Grand Prix')
    })
  })
}
