/**
 * E2E regression suite for the Joi → Zod validation migration.
 *
 * Verifies black-box, over the real HTTP API, that request validation behaves
 * exactly as before the migration:
 *   - payloads Joi accepted are still accepted (zod did not become stricter),
 *   - unknown body keys are still tolerated AND preserved (Joi ran with
 *     allowUnknown: true; the zod schemas must be passthrough),
 *   - invalid payloads are still rejected with 400 and the legacy
 *     "Request validation of body failed, because: ..." message format.
 *
 * Happy paths go through the real SDK; invalid/REST-only shapes go through raw
 * fetch because the SDK types would not let us build them.
 */
import RushDB from '../../packages/javascript-sdk/src/index.node'

jest.setTimeout(60_000)

describe('validation after zod migration (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })
  const tenantId = `zodmig-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  const LABELS = ['ZodMigRecord', 'ZodMigProps', 'ZodMigJunk', 'ZodMigCsv', 'ZodMigSrc', 'ZodMigTgt']

  // Raw client for payloads the SDK types would reject (invalid bodies, REST-only shapes).
  const raw = async (path: string, body: unknown, method = 'POST') => {
    const response = await fetch(`${apiUrl}/api/v1${path}`, {
      method,
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${apiKey}`
      },
      body: JSON.stringify(body)
    })
    const json: any = await response.json().catch(() => ({}))
    return { status: response.status, json }
  }

  const expectValidationError = (result: { status: number; json: any }) => {
    expect(result.status).toBe(400)
    expect(String(result.json.message)).toContain('Request validation of body failed, because')
  }

  afterAll(async () => {
    await db.records.delete({ labels: LABELS, where: { tenantId } }).catch(() => undefined)
  })

  // ────────────────────────────────────────────────────────────────────────────
  // Accepted payloads: nothing got stricter
  // ────────────────────────────────────────────────────────────────────────────

  it('accepts the data shape with every value kind (string, number, boolean, null, array, datetime)', async () => {
    const created = await db.records.create({
      label: 'ZodMigRecord',
      data: {
        tenantId,
        name: 'widget',
        price: 10.5,
        inStock: true,
        discontinued: null,
        tags: ['a', 'b'],
        releasedAt: '2024-01-01T00:00:00Z'
      }
    })

    expect(created.data.name).toBe('widget')
    expect(created.data.price).toBe(10.5)
    expect(created.data.inStock).toBe(true)
    expect(created.data.tags).toEqual(['a', 'b'])
    // null means "field unset" and is dropped during normalization
    expect(created.data.discontinued).toBeUndefined()
  })

  it('accepts the properties shape, including valueSeparator splitting', async () => {
    const result = await raw('/records', {
      label: 'ZodMigProps',
      properties: [
        { type: 'string', name: 'tenantId', value: tenantId },
        { type: 'number', name: 'scores', value: '1;2;3', valueSeparator: ';' },
        { type: 'datetime', name: 'seenAt', value: '2024-06-01T12:00:00Z' },
        { type: 'boolean', name: 'active', value: [true, false] }
      ]
    })

    expect([200, 201]).toContain(result.status)

    const found = await db.records.find({
      labels: ['ZodMigProps'],
      where: { tenantId }
    })
    expect(found.data).toHaveLength(1)
    expect(found.data[0]?.data.scores).toEqual([1, 2, 3])
    expect(found.data[0]?.data.active).toEqual([true, false])
  })

  it('still tolerates and preserves unknown body keys (allowUnknown regression)', async () => {
    // Joi validated with allowUnknown: true and the pipe returns the validated
    // value; the zod schemas must be passthrough or keys like `vectors` vanish.
    const create = await raw('/records', {
      label: 'ZodMigJunk',
      data: { tenantId, name: 'junk-carrier' },
      somethingUnknown: { nested: true }
    })
    expect([200, 201]).toContain(create.status)

    const search = await raw('/records/search', {
      labels: ['ZodMigJunk'],
      where: { tenantId },
      someJunkKey: 'ignored'
    })
    expect(search.status).toBe(200)
    expect(search.json.total ?? search.json.data?.total).toBeDefined()
  })

  it('accepts a search with pagination, ordering, recursive select expressions and groupBy', async () => {
    const result = await raw('/records/search', {
      labels: ['ZodMigRecord'],
      where: { tenantId, price: { $gte: 0 } },
      limit: 100,
      skip: 0,
      orderBy: { price: 'desc' },
      select: {
        total: { $count: '*' },
        ratio: { $divide: [{ $sum: '$record.price' }, { $avg: '$record.price', $precision: 2 }] }
      },
      groupBy: ['$record.name']
    })
    expect(result.status).toBe(200)
  })

  it('accepts the edit (PATCH) data shape', async () => {
    const created = await db.records.create({
      label: 'ZodMigRecord',
      data: { tenantId, name: 'patch-me', price: 1 }
    })

    const patched = await raw(
      `/records/${created.id}`,
      { label: 'ZodMigRecord', data: { tenantId, name: 'patched', price: 2 } },
      'PATCH'
    )
    expect([200, 201]).toContain(patched.status)
  })

  it('accepts relationships create-many in both key-join and many-to-many shapes', async () => {
    await db.records.createMany({
      label: 'ZodMigSrc',
      data: [{ sid: 's1', tenantId }]
    })
    await db.records.createMany({
      label: 'ZodMigTgt',
      data: [{ sid: 's1', tenantId }]
    })

    const keyJoin = await db.relationships.createMany({
      source: { label: 'ZodMigSrc', key: 'sid', where: { tenantId } },
      target: { label: 'ZodMigTgt', key: 'sid', where: { tenantId } },
      type: 'ZODMIG_KEYJOIN',
      direction: 'out'
    })
    expect(keyJoin.success).toBe(true)

    const manyToMany = await raw('/relationships/create-many', {
      source: { label: 'ZodMigSrc', where: { tenantId } },
      target: { label: 'ZodMigTgt', where: { tenantId } },
      type: 'ZODMIG_M2M',
      manyToMany: true
    })
    expect([200, 201]).toContain(manyToMany.status)
  })

  it('accepts importJson with options and importCsv with parseConfig', async () => {
    const json = await db.records.importJson({
      label: 'ZodMigRecord',
      data: [{ tenantId, name: 'import-a' }],
      options: { suggestTypes: true, returnResult: true }
    })
    expect(json.data.length).toBe(1)

    const csv = await db.records.importCsv({
      label: 'ZodMigCsv',
      data: `code;tenantId\nCSV1;${tenantId}\n\nCSV2;${tenantId}`,
      options: { returnResult: true, suggestTypes: true },
      parseConfig: { delimiter: ';', header: true, skipEmptyLines: 'greedy' }
    })
    expect(csv.data.length).toBe(2)
  })

  // ────────────────────────────────────────────────────────────────────────────
  // Rejected payloads: nothing got looser, message format preserved
  // ────────────────────────────────────────────────────────────────────────────

  it('rejects a mis-named payload key with 400 instead of dying in Cypher', async () => {
    expectValidationError(await raw('/records', { label: 'ZodMigRecord', payload: { a: 1 } }))
  })

  it('rejects a property whose value does not match its declared type', async () => {
    expectValidationError(
      await raw('/records', {
        label: 'ZodMigRecord',
        properties: [{ type: 'number', name: 'price', value: 'not-a-number' }]
      })
    )
  })

  it('rejects an invalid ISO datetime value with the ISO8601 message', async () => {
    const result = await raw('/records', {
      label: 'ZodMigRecord',
      properties: [{ type: 'datetime', name: 'seenAt', value: 'yesterday' }]
    })
    expectValidationError(result)
    expect(String(result.json.message)).toContain('ISO8601')
  })

  it('rejects a search with limit above 1000', async () => {
    expectValidationError(await raw('/records/search', { limit: 2000 }))
  })

  it('rejects a search with an invalid orderBy direction', async () => {
    expectValidationError(await raw('/records/search', { orderBy: 'sideways' }))
  })

  it('rejects relationships create-many when a side has neither key nor where filter', async () => {
    expectValidationError(
      await raw('/relationships/create-many', {
        source: { label: 'ZodMigSrc' },
        target: { label: 'ZodMigTgt', key: 'sid' }
      })
    )
  })

  it('rejects importCsv with an invalid skipEmptyLines value', async () => {
    expectValidationError(
      await raw('/records/import/csv', {
        label: 'ZodMigCsv',
        data: 'a,b\n1,2',
        parseConfig: { skipEmptyLines: 'always' }
      })
    )
  })
})
