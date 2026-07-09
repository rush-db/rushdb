import RushDB from '../../packages/javascript-sdk/src/index.node'

jest.setTimeout(120_000)

describe('db.ai External Vector flow (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })

  const tenantId = `ai-external-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  const LABEL = 'ExternalArticle'
  const PROPERTY = 'description'

  const vectors = {
    alpha: [1, 0, 0],
    beta: [0, 1, 0],
    gamma: [0, 0, 1]
  }

  let indexId: string | undefined
  let recordIds: string[] = []

  beforeAll(async () => {
    const alpha = await db.records.create({
      label: LABEL,
      data: {
        title: 'Alpha',
        description: 'alpha text',
        tenantId
      },
      options: { suggestTypes: true }
    })

    const beta = await db.records.create({
      label: LABEL,
      data: {
        title: 'Beta',
        description: 'beta text',
        tenantId
      },
      options: { suggestTypes: true }
    })

    const gamma = await db.records.create({
      label: LABEL,
      data: {
        title: 'Gamma',
        description: 'gamma text',
        tenantId
      },
      options: { suggestTypes: true }
    })

    recordIds = [alpha.id, beta.id, gamma.id].filter(Boolean)

    const index = await db.ai.indexes.create({
      label: LABEL,
      propertyName: PROPERTY,
      sourceType: 'external',
      similarityFunction: 'cosine',
      dimensions: 3
    })

    indexId = index.data.id

    await db.ai.indexes.upsertVectors(indexId, {
      items: [
        { recordId: alpha.id, vector: vectors.alpha },
        { recordId: beta.id, vector: vectors.beta },
        { recordId: gamma.id, vector: vectors.gamma }
      ]
    })
  })

  afterAll(async () => {
    await db.records.delete({ labels: [LABEL], where: { tenantId } })

    if (indexId) {
      try {
        await db.ai.indexes.delete(indexId)
      } catch {
        // ignore
      }
    }
  })

  it('supports create(external) + upsertVectors + queryVector search', async () => {
    expect(indexId).toBeDefined()
    expect(recordIds.length).toBe(3)

    const res = await db.records.vectorSearch({
      propertyName: PROPERTY,
      labels: [LABEL],
      sourceType: 'external',
      similarityFunction: 'cosine',
      dimensions: 3,
      queryVector: [1, 0, 0],
      where: { tenantId },
      limit: 3
    })

    expect(res.data.length).toBeGreaterThan(0)
    expect(String(res.data[0].data.title ?? '')).toBe('Alpha')

    for (let i = 0; i < res.data.length - 1; i++) {
      expect(res.data[i].data.__score).toBeGreaterThanOrEqual(res.data[i + 1].data.__score)
    }
  })

  it('rejects query text for external index', async () => {
    await expect(
      db.records.vectorSearch({
        propertyName: PROPERTY,
        labels: [LABEL],
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: 3,
        query: 'alpha text',
        where: { tenantId },
        limit: 3
      })
    ).rejects.toBeTruthy()
  })
})
