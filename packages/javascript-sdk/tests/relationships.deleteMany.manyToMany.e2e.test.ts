import path from 'path'
import dotenv from 'dotenv'

// Load .env from the javascript-sdk package folder
dotenv.config({ path: path.resolve(__dirname, '../.env') })

import RushDB from '../src/index.node'

const sleep = (ms: number) => new Promise((res) => setTimeout(res, ms))

describe('relationships.deleteMany manyToMany (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: process.env.RUSHDB_API_URL })

  const tenantId = `sdk-e2e-del-mtm-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

  jest.setTimeout(120_000)

  it('deletes many-to-many relations when explicitly requested', async () => {
    const users = [
      { id: 'mdu1', tenantId },
      { id: 'mdu2', tenantId }
    ]
    const tags = [
      { id: 'mdt1', tenantId },
      { id: 'mdt2', tenantId }
    ]

    await db.records.createMany({ label: 'USER_DEL_MTM', data: users, options: { returnResult: false } })
    await db.records.createMany({ label: 'TAG_DEL_MTM', data: tags, options: { returnResult: false } })

    await db.relationships.createMany({
      source: { label: 'USER_DEL_MTM', where: { tenantId } },
      target: { label: 'TAG_DEL_MTM', where: { tenantId } },
      type: 'HAS_TAG',
      direction: 'out',
      manyToMany: true
    })

    // wait for creation
    await sleep(2000)

    let delRes
    try {
      delRes = await db.relationships.deleteMany({
        source: { label: 'USER_DEL_MTM', where: { tenantId } },
        target: { label: 'TAG_DEL_MTM', where: { tenantId } },
        type: 'HAS_TAG',
        direction: 'out',
        manyToMany: true
      })
    } catch (err: any) {
      // Print full error details for debugging (AggregateError may wrap multiple errors)
      // eslint-disable-next-line no-console
      console.error('deleteMany (manyToMany) error:', err && err.message ? err.message : err)
      // eslint-disable-next-line no-console
      console.error('Error stack:', err && err.stack ? err.stack : '(no stack)')
      if (err && err.errors && Array.isArray(err.errors)) {
        // eslint-disable-next-line no-console
        console.error(
          'AggregateError inner errors:',
          err.errors.map((e: any) => ({
            message: e.message,
            stack: e.stack,
            name: e.name,
            code: e.code,
            response:
              e.response && typeof e.response === 'object' ? e.response.body || e.response : e.response
          }))
        )
      }
      if (err && err.response) {
        try {
          // eslint-disable-next-line no-console
          console.error('Error response body:', JSON.stringify(err.response.body || err.response, null, 2))
        } catch (e) {
          // eslint-disable-next-line no-console
          console.error('Could not stringify response body', e)
        }
      }
      throw err
    }

    expect(delRes.success).toBe(true)

    // verify deletion
    let attempts = 0
    let matchedCount = 0
    while (attempts < 8) {
      const findRes = await db.relationships.find({
        labels: ['USER_DEL_MTM'],
        where: { tenantId },
        limit: 1000,
        skip: 0
      })
      expect(findRes.success).toBe(true)
      const data = (findRes.data || []) as Array<{ type: string }>
      matchedCount = data.filter((r) => r.type === 'HAS_TAG').length
      if (matchedCount === 0) break
      attempts += 1
      await sleep(1000)
    }

    expect(matchedCount).toBe(0)
  })
})
