import path from 'path'
import dotenv from 'dotenv'

dotenv.config({ path: path.resolve(__dirname, '../.env') })

import RushDB from '../src/index.node'

jest.setTimeout(60_000)

describe('records.importJson upsert nested linking (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })

  it('reuses child record and links it to new parent when parent upsert creates a new record', async () => {
    const tenantId = `nested-upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    const payloadA = {
      name: 'Abshire - Farrell',
      address: '928 Conroy Village Suite 785',
      foundedAt: '1949-10-06T22:07:28.709Z',
      rating: 1.9,
      tenantId,
      department: [
        {
          name: 'Sports',
          description: 'The sleek and filthy Gloves comes with sky blue LED lighting for smart functionality',
          tenantId
        }
      ]
    }

    const payloadB = {
      ...payloadA,
      rating: 2 // slight change should force new top-level record when mergeBy is all keys
    }

    // First import creates parent and child
    await db.records.importJson({ label: 'Company', data: payloadA, options: { suggestTypes: true } })

    // Second import should create new parent record but reuse existing child and link it
    await db.records.importJson({
      label: 'Company',
      data: payloadB,
      options: { suggestTypes: true, mergeBy: [] }
    })

    // Verify there are two Company records and one Department record linked to both via default relation
    const companies = await db.records.find({ labels: ['Company'], where: { tenantId } })
    expect(companies.total).toBe(2)

    const departments = await db.records.find({ labels: ['department'], where: { tenantId } })
    // label normalization in service uses original key; depending on capitalization option it might be 'department'
    // Allow 1 department entry
    expect(departments.total).toBe(1)

    // Fetch relations and ensure both companies are connected to the same department
    const relationsResp = await db.relationships.find({})
    const rels = relationsResp.data.filter(
      (r) =>
        r.type &&
        (r.type.includes('RUSHDB_DEFAULT_RELATION') || r.type.includes('__RUSHDB__RELATION__DEFAULT__'))
    )

    const departmentId = departments.data[0].id()
    const companyIds = companies.data.map((c) => c.id())

    // For each company, there must be at least one relation to the department (either direction)
    const relatedPairs = new Set(rels.map((r) => `${r.sourceId}->${r.targetId}`))
    for (const cid of companyIds) {
      const has = relatedPairs.has(`${cid}->${departmentId}`) || relatedPairs.has(`${departmentId}->${cid}`)
      expect(has).toBe(true)
    }

    // Cleanup
    await db.records.delete({ where: { tenantId } })
  })
})
