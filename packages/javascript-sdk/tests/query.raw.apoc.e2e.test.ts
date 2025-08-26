import path from 'path'
import dotenv from 'dotenv'

// Load .env from the javascript-sdk package folder
dotenv.config({ path: path.resolve(__dirname, '../.env') })

import RushDB from '../src/index.node'

jest.setTimeout(30_000)

describe('query.raw apoc (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })

  it('executes apoc functions and returns converted values', async () => {
    // Use a deterministic query that calls apoc functions
    const query = `
      RETURN apoc.coll.sort([3,1,2]) AS sorted, apoc.text.join(['a','b','c'], '-') AS joined
    `

    const res = await db.query.raw({ query, params: {} })
    // eslint-disable-next-line no-console
    console.debug('APOC RAW RESPONSE:', JSON.stringify(res, null, 2))

    const payload = res.data
    const records = Array.isArray(payload) ? payload : (payload?.records ?? [])

    expect(Array.isArray(records)).toBe(true)
    expect(records.length).toBeGreaterThanOrEqual(1)

    const first = records[0]

    // Depending on the shaping, the record may be { sorted: [...], joined: 'a-b-c' }
    // or { 'sorted': [...], 'joined': 'a-b-c' } - assert both values are present
    const sorted = first?.sorted ?? first?.['sorted']
    const joined = first?.joined ?? first?.['joined']

    expect(Array.isArray(sorted)).toBe(true)
    expect(sorted).toEqual([1, 2, 3])
    expect(joined).toBe('a-b-c')
  })
})
