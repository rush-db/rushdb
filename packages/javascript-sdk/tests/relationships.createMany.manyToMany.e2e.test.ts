import path from 'path'
import dotenv from 'dotenv'

// Load .env from the javascript-sdk package folder
dotenv.config({ path: path.resolve(__dirname, '../.env') })

import RushDB from '../src/index.node'

const sleep = (ms: number) => new Promise((res) => setTimeout(res, ms))

describe('relationships.createMany manyToMany (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: process.env.RUSHDB_API_URL })

  const tenantId = `sdk-e2e-mtm-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

  jest.setTimeout(120_000)

  it('creates many-to-many relations between two label sets when explicitly requested', async () => {
    // Create three users and two tags; expect 3*2 = 6 links when manyToMany true
    const users = [
      { id: 'mu1', tenantId },
      { id: 'mu2', tenantId },
      { id: 'mu3', tenantId }
    ]
    const tags = [
      { id: 't1', tenantId },
      { id: 't2', tenantId }
    ]

    await db.records.createMany({ label: 'USER_MTM', data: users, options: { returnResult: false } })
    await db.records.createMany({ label: 'TAG_MTM', data: tags, options: { returnResult: false } })

    const createRes = await db.relationships.createMany({
      source: { label: 'USER_MTM', where: { tenantId } },
      target: { label: 'TAG_MTM', where: { tenantId } },
      type: 'HAS_TAG',
      direction: 'out',
      manyToMany: true
    })

    expect(createRes.success).toBe(true)

    // wait and verify count
    let attempts = 0
    let matchedCount = 0

    while (attempts < 8) {
      const findRes = await db.relationships.find({
        labels: ['USER_MTM'],
        where: { tenantId },
        limit: 1000,
        skip: 0
      })
      expect(findRes.success).toBe(true)

      const data = (findRes.data || []) as Array<{ type: string }>
      matchedCount = data.filter((r) => r.type === 'HAS_TAG').length
      if (matchedCount >= 6) break
      attempts += 1
      await sleep(1000)
    }

    expect(matchedCount).toBeGreaterThanOrEqual(6)
  })
})
