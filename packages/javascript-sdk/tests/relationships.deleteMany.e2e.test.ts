import path from 'path'
import dotenv from 'dotenv'

// Load .env from the javascript-sdk package folder
dotenv.config({ path: path.resolve(__dirname, '../.env') })

import RushDB from '../src/index.node'

const sleep = (ms: number) => new Promise((res) => setTimeout(res, ms))

describe('relationships.deleteMany (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: process.env.RUSHDB_API_URL })

  const tenantId = `sdk-e2e-del-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

  const users = [
    { id: 'du1', tenantId },
    { id: 'du2', tenantId }
  ]
  const orders = [
    { id: 'do1', userId: 'du1', tenantId },
    { id: 'do2', userId: 'du2', tenantId }
  ]

  jest.setTimeout(120_000)

  it('deletes relations between USER and ORDER by key match', async () => {
    // create data and create relations first
    await db.records.createMany({ label: 'USER_DEL', data: users, options: { returnResult: false } })
    await db.records.createMany({ label: 'ORDER_DEL', data: orders, options: { returnResult: false } })

    await db.relationships.createMany({
      source: { label: 'USER_DEL', key: 'id', where: { tenantId } },
      target: { label: 'ORDER_DEL', key: 'userId', where: { tenantId } },
      type: 'ORDERED',
      direction: 'out'
    })

    // wait for creation
    await sleep(2000)

    // now delete them
    const delRes = await db.relationships.deleteMany({
      source: { label: 'USER_DEL', key: 'id', where: { tenantId } },
      target: { label: 'ORDER_DEL', key: 'userId', where: { tenantId } },
      type: 'ORDERED',
      direction: 'out'
    })

    expect(delRes.success).toBe(true)

    // verify deletion by checking relationships.find returns none of that type
    let attempts = 0
    let matchedCount = 0
    while (attempts < 8) {
      const findRes = await db.relationships.find({
        labels: ['USER_DEL'],
        where: { tenantId },
        limit: 1000,
        skip: 0
      })
      expect(findRes.success).toBe(true)
      const data = (findRes.data || []) as Array<{ type: string }>
      matchedCount = data.filter((r) => r.type === 'ORDERED').length
      if (matchedCount === 0) break
      attempts += 1
      await sleep(1000)
    }

    expect(matchedCount).toBe(0)
  })
})
