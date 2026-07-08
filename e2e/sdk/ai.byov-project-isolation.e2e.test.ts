/**
 * BYOV project-isolation e2e coverage.
 *
 * This suite uses two real projects with two API tokens and the same label/property
 * pair in both projects. It verifies that external vector indexes and semantic
 * search candidates are scoped by project, so a `description` index in project A
 * never leaks into project B when project B searches its own `description` index.
 */

import RushDB from '../../packages/javascript-sdk/src/index.node'
import type { EmbeddingIndex } from '../../packages/javascript-sdk/src/api/types'
import { ADMIN_LOGIN, ADMIN_PASSWORD, BASE_URL } from '../setup/env'

jest.setTimeout(120_000)

const API = `${BASE_URL}/api/v1`
const RUN = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

type RawResponse = { status: number; body: any }

const apiRaw = async (
  path: string,
  {
    method = 'GET',
    token,
    headers = {},
    body
  }: { method?: string; token?: string; headers?: Record<string, string>; body?: unknown } = {}
): Promise<RawResponse> => {
  const response = await fetch(`${API}${path}`, {
    method,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...headers
    },
    ...(body !== undefined ? { body: JSON.stringify(body) } : {})
  })
  const json: any = await response.json().catch(() => ({}))
  return { status: response.status, body: json }
}

const api = async (path: string, options: Parameters<typeof apiRaw>[1] = {}) => {
  const { status, body } = await apiRaw(path, options)
  if (status < 200 || status >= 300) {
    throw new Error(`${options.method ?? 'GET'} ${path} -> ${status}: ${JSON.stringify(body)}`)
  }
  return body.data ?? body
}

const unitVec = (i: number, dims = 4): number[] => Array.from({ length: dims }, (_, k) => (k === i ? 1 : 0))

async function waitForIndexReady(
  db: RushDB,
  indexId: string,
  timeoutMs = 60_000,
  interval = 2_000
): Promise<void> {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    const list = await db.ai.indexes.find()
    const idx = (list.data as EmbeddingIndex[]).find((i) => i.id === indexId)
    if (idx?.status === 'ready') return
    if (idx?.status === 'error') throw new Error(`Embedding index ${indexId} entered error state`)
    await new Promise((r) => setTimeout(r, interval))
  }
  throw new Error(`Embedding index ${indexId} did not become ready within ${timeoutMs}ms`)
}

describe('BYOV semantic indexes are project isolated (e2e)', () => {
  const apiUrl = process.env.RUSHDB_API_URL || BASE_URL

  const LABEL = `ByovSharedDoc_${RUN.replace(/-/g, '_')}`
  const PROPERTY = 'description'
  const DIMS = 4

  let tokenA: string
  let tokenB: string
  let dbA: RushDB
  let dbB: RushDB
  let indexAId: string | undefined
  let indexBId: string | undefined

  const createProjectWithToken = async (jwt: string, workspaceId: string, name: string): Promise<string> => {
    const project = await api('/projects', {
      method: 'POST',
      token: jwt,
      headers: { 'x-workspace-id': workspaceId },
      body: { name, description: 'BYOV project isolation e2e' }
    })
    const token = await api('/tokens', {
      method: 'POST',
      token: jwt,
      headers: { 'x-project-id': project.id, 'x-workspace-id': workspaceId },
      body: { name: 'e2e-byov', expiration: '1d' }
    })
    return token.value
  }

  beforeAll(async () => {
    const user = await api('/auth/login', {
      method: 'POST',
      body: { login: ADMIN_LOGIN, password: ADMIN_PASSWORD }
    })
    const workspaces = await api('/workspaces', { token: user.token })
    const workspaceId: string = workspaces[0]?.id
    expect(workspaceId).toBeTruthy()

    tokenA = await createProjectWithToken(user.token, workspaceId, `e2e-byov-a-${RUN}`)
    tokenB = await createProjectWithToken(user.token, workspaceId, `e2e-byov-b-${RUN}`)
    dbA = new RushDB(tokenA, { url: apiUrl })
    dbB = new RushDB(tokenB, { url: apiUrl })
  })

  afterAll(async () => {
    if (dbA) {
      await dbA.records.delete({ labels: [LABEL] }).catch(() => {})
      if (indexAId) await dbA.ai.indexes.delete(indexAId).catch(() => {})
    }
    if (dbB) {
      await dbB.records.delete({ labels: [LABEL] }).catch(() => {})
      if (indexBId) await dbB.ai.indexes.delete(indexBId).catch(() => {})
    }
  })

  it('supports end-to-end BYOV writes and vector search in two projects with the same label/property', async () => {
    // Seed the same label/property in both projects so each project can create
    // an external index with the exact same semantic key.
    await Promise.all([
      dbA.records.create({ label: LABEL, data: { description: 'project-a schema seed', owner: 'userA-seed' } }),
      dbB.records.create({ label: LABEL, data: { description: 'project-b schema seed', owner: 'userB-seed' } })
    ])

    const [{ data: indexA }, { data: indexB }] = await Promise.all([
      dbA.ai.indexes.create({
        label: LABEL,
        propertyName: PROPERTY,
        external: true,
        dimensions: DIMS,
        similarityFunction: 'cosine'
      }),
      dbB.ai.indexes.create({
        label: LABEL,
        propertyName: PROPERTY,
        external: true,
        dimensions: DIMS,
        similarityFunction: 'cosine'
      })
    ])
    indexAId = indexA.id
    indexBId = indexB.id

    const projectARecord = await dbA.records.create({
      label: LABEL,
      data: { description: 'shared property text that belongs only to project A', owner: 'userA' },
      vectors: [{ propertyName: PROPERTY, vector: unitVec(0), dimensions: DIMS, similarityFunction: 'cosine' }]
    })

    await dbB.records.createMany({
      label: LABEL,
      data: [
        { description: 'shared property text that belongs only to project B', owner: 'userB-primary' },
        { description: 'secondary project B document', owner: 'userB-secondary' }
      ],
      vectors: [
        [{ propertyName: PROPERTY, vector: unitVec(1), dimensions: DIMS, similarityFunction: 'cosine' }],
        [{ propertyName: PROPERTY, vector: unitVec(2), dimensions: DIMS, similarityFunction: 'cosine' }]
      ]
    })

    // Back-fill the schema seed records so both external indexes become ready.
    const [seedA, seedB] = await Promise.all([
      dbA.records.find({ labels: [LABEL], where: { description: 'project-a schema seed' } }),
      dbB.records.find({ labels: [LABEL], where: { description: 'project-b schema seed' } })
    ])
    await Promise.all([
      dbA.ai.indexes.upsertVectors(indexAId, {
        items: [{ recordId: seedA.data[0].id, vector: unitVec(3) }]
      }),
      dbB.ai.indexes.upsertVectors(indexBId, {
        items: [{ recordId: seedB.data[0].id, vector: unitVec(3) }]
      })
    ])

    await Promise.all([waitForIndexReady(dbA, indexAId), waitForIndexReady(dbB, indexBId)])

    const projectAResults = await dbA.ai.search({
      propertyName: PROPERTY,
      labels: [LABEL],
      sourceType: 'external',
      similarityFunction: 'cosine',
      dimensions: DIMS,
      queryVector: unitVec(0),
      limit: 5
    })

    expect(projectAResults.data[0]?.data.__id).toBe(projectARecord.id)
    expect(projectAResults.data.map((record) => record.data.owner)).not.toContain('userB-primary')

    const projectBResultsForAQuery = await dbB.ai.search({
      propertyName: PROPERTY,
      labels: [LABEL],
      sourceType: 'external',
      similarityFunction: 'cosine',
      dimensions: DIMS,
      queryVector: unitVec(0),
      limit: 5
    })

    // Project B is allowed to search its own same-named `description` index, but
    // project A's perfect vector match must never appear in project B results.
    expect(projectBResultsForAQuery.data.map((record) => record.data.__id)).not.toContain(projectARecord.id)
    expect(projectBResultsForAQuery.data.every((record) => String(record.data.owner ?? '').startsWith('userB'))).toBe(
      true
    )

    const projectBResults = await dbB.ai.search({
      propertyName: PROPERTY,
      labels: [LABEL],
      sourceType: 'external',
      similarityFunction: 'cosine',
      dimensions: DIMS,
      queryVector: unitVec(1),
      limit: 5
    })

    expect(projectBResults.data[0]?.data.owner).toBe('userB-primary')
    expect(projectBResults.data[0]?.data.__score).toBeCloseTo(1, 2)
  })

  it('does not let project B use project A as the matching index when project B has no index', async () => {
    const noIndexLabel = `${LABEL}_NoIndex`

    await dbA.records.create({ label: noIndexLabel, data: { description: 'project A only seed' } })
    const { data: projectAOnlyIndex } = await dbA.ai.indexes.create({
      label: noIndexLabel,
      propertyName: PROPERTY,
      external: true,
      dimensions: DIMS,
      similarityFunction: 'cosine'
    })

    try {
      const aRecord = await dbA.records.create({
        label: noIndexLabel,
        data: { description: 'project A only indexed document' },
        vectors: [{ propertyName: PROPERTY, vector: unitVec(0), dimensions: DIMS, similarityFunction: 'cosine' }]
      })
      const seedA = await dbA.records.find({
        labels: [noIndexLabel],
        where: { description: 'project A only seed' }
      })
      await dbA.ai.indexes.upsertVectors(projectAOnlyIndex.id, {
        items: [{ recordId: seedA.data[0].id, vector: unitVec(1) }]
      })
      await waitForIndexReady(dbA, projectAOnlyIndex.id)

      await dbB.records.create({
        label: noIndexLabel,
        data: { description: 'project B same label and property but no index' }
      })

      await expect(
        dbB.ai.search({
          propertyName: PROPERTY,
          labels: [noIndexLabel],
          sourceType: 'external',
          similarityFunction: 'cosine',
          dimensions: DIMS,
          queryVector: unitVec(0),
          limit: 5
        })
      ).rejects.toBeTruthy()

      const ownerResults = await dbA.ai.search({
        propertyName: PROPERTY,
        labels: [noIndexLabel],
        sourceType: 'external',
        similarityFunction: 'cosine',
        dimensions: DIMS,
        queryVector: unitVec(0),
        limit: 5
      })
      expect(ownerResults.data[0]?.data.__id).toBe(aRecord.id)
    } finally {
      await dbA.records.delete({ labels: [noIndexLabel] }).catch(() => {})
      await dbB.records.delete({ labels: [noIndexLabel] }).catch(() => {})
      await dbA.ai.indexes.delete(projectAOnlyIndex.id).catch(() => {})
    }
  })
})
