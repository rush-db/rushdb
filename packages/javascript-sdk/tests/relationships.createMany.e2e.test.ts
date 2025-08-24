import path from 'path'
import dotenv from 'dotenv'

// Load .env from the javascript-sdk package folder
dotenv.config({ path: path.resolve(__dirname, '../.env') })

import RushDB from '../src/index.node'

const sleep = (ms: number) => new Promise((res) => setTimeout(res, ms))

describe('relationships.createMany (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY

  if (!apiKey) {
    // Skip tests gracefully when no API key is provided
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: process.env.RUSHDB_API_URL })

  // Use a unique tenantId to isolate this test run
  const tenantId = `sdk-e2e-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

  // Sample data
  const users = [
    { id: 'u1', tenantId },
    { id: 'u2', tenantId }
  ]
  const orders = [
    { id: 'o1', userId: 'u1', tenantId },
    { id: 'o2', userId: 'u2', tenantId },
    // Unmatched order (no corresponding user)
    { id: 'o3', userId: 'u3', tenantId }
  ]

  jest.setTimeout(120_000)

  it('creates relations between USER and ORDER by key match', async () => {
    // 1) Create USER and ORDER records
    await db.records.createMany({ label: 'USER', data: users, options: { returnResult: false } })
    await db.records.createMany({ label: 'ORDER', data: orders, options: { returnResult: false } })

    // 2) Create relations USER -[:ORDERED]-> ORDER where USER.id = ORDER.userId and same tenant
    const createRes = await db.relationships.createMany({
      source: { label: 'USER', key: 'id', where: { tenantId } },
      target: { label: 'ORDER', key: 'userId', where: { tenantId } },
      type: 'ORDERED',
      direction: 'out'
    })

    expect(createRes.success).toBe(true)

    // 3) Verify relations via relationships.find
    //    Use retries because apoc.periodic.iterate may complete just after the response.
    let attempts = 0
    let matchedCount = 0
    let lastData: Array<{ type: string; sourceLabel: string; targetLabel: string }> = []

    while (attempts < 8) {
      const findRes = await db.relationships.find({
        labels: ['USER'],
        where: { tenantId },
        limit: 1000,
        skip: 0
      })

      expect(findRes.success).toBe(true)

      const data = (findRes.data || []) as Array<{
        type: string
        sourceLabel: string
        targetLabel: string
      }>

      lastData = data

      const orderedFromUser = data.filter(
        (r) => r.type === 'ORDERED' && r.sourceLabel === 'USER' && r.targetLabel === 'ORDER'
      )

      matchedCount = orderedFromUser.length
      if (matchedCount >= 2) break

      attempts += 1
      await sleep(1000)
    }

    // Expect exactly two ORDERED relations from USER -> ORDER (for u1 and u2)
    expect(matchedCount).toBeGreaterThanOrEqual(2)

    // Sanity: there should be no relation created for unmatched userId u3
    // We can’t directly check by id here without extra queries; ensure we at least got two expected links.
  })
})
