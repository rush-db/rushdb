/**
 * Cross-tenant isolation, black-box over the real HTTP API with two projects and two
 * API tokens. This is the safety net for removing `IsRelatedToProjectGuard`: every
 * by-id route must enforce project scoping in its Cypher (`{ projectId: $projectId }`),
 * so ids from another project never match — reads 404, writes no-op.
 *
 * Covers the routes the guard used to sit on:
 *   records:       GET / PATCH / PUT / DELETE /:id, POST /delete, POST /:id/search
 *   relationships: POST /:id (attach), PUT /:id (detach) — foreign source and target
 *   properties:    GET /:id, POST /:id/values, DELETE /:id (previously guard-protected)
 */
import { ADMIN_LOGIN, ADMIN_PASSWORD, BASE_URL } from './setup/env'

const API = `${BASE_URL}/api/v1`
// Unique per run so the suite can be re-run against a persistent external stack.
const RUN = Date.now().toString(36)
const LABEL_A = `E2E_ISO_A_${RUN}`
const LABEL_B = `E2E_ISO_B_${RUN}`

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

const stringProp = (name: string, value: string) => ({ name, type: 'string', value })

describe('tenant isolation without IsRelatedToProjectGuard (e2e)', () => {
  let tokenA: string
  let tokenB: string

  // Records living in project A
  let recA1: string
  let recA2: string
  // Record living in project B (used as a legitimate source for cross-tenant attach)
  let recB1: string
  // A property node of project A (the `secret` property)
  let propA: string

  const createProjectWithToken = async (
    jwt: string,
    workspaceId: string,
    name: string
  ): Promise<string> => {
    const project = await api('/projects', {
      method: 'POST',
      token: jwt,
      headers: { 'x-workspace-id': workspaceId },
      body: { name, description: 'tenant isolation e2e' }
    })
    const token = await api('/tokens', {
      method: 'POST',
      token: jwt,
      headers: { 'x-project-id': project.id, 'x-workspace-id': workspaceId },
      body: { name: 'e2e', expiration: '1d' }
    })
    return token.value
  }

  beforeAll(async () => {
    // Self-hosted boot auto-creates the admin user and Default Workspace.
    const user = await api('/auth/login', {
      method: 'POST',
      body: { login: ADMIN_LOGIN, password: ADMIN_PASSWORD }
    })
    const workspaces = await api('/workspaces', { token: user.token })
    const workspaceId: string = workspaces[0]?.id
    expect(workspaceId).toBeTruthy()

    tokenA = await createProjectWithToken(user.token, workspaceId, `e2e-iso-a-${RUN}`)
    tokenB = await createProjectWithToken(user.token, workspaceId, `e2e-iso-b-${RUN}`)

    const a1 = await api('/records', {
      method: 'POST',
      token: tokenA,
      body: {
        label: LABEL_A,
        properties: [stringProp('name', 'Alpha'), stringProp('secret', 'a-secret')]
      }
    })
    recA1 = a1.__id
    const a2 = await api('/records', {
      method: 'POST',
      token: tokenA,
      body: { label: LABEL_A, properties: [stringProp('name', 'Beta')] }
    })
    recA2 = a2.__id

    const b1 = await api('/records', {
      method: 'POST',
      token: tokenB,
      body: { label: LABEL_B, properties: [stringProp('name', 'Bravo')] }
    })
    recB1 = b1.__id
    expect(recA1 && recA2 && recB1).toBeTruthy()

    const propsA = await api('/properties/search', { method: 'POST', token: tokenA, body: {} })
    propA = propsA.find((p: any) => p.name === 'secret')?.id
    expect(propA).toBeTruthy()

    // Baseline relationship inside project A (detach isolation is asserted against it).
    await api(`/relationships/${recA1}`, {
      method: 'POST',
      token: tokenA,
      body: { targetIds: [recA2] }
    })
  })

  const getRecord = (id: string, token: string) => apiRaw(`/records/${id}`, { token })

  // The response envelope is { data, total, success } — total lives beside data.
  const relationsCount = async (token: string): Promise<number> => {
    const { status, body } = await apiRaw('/relationships/search', { method: 'POST', token, body: {} })
    expect(status).toBe(200)
    return body.total
  }

  describe('records', () => {
    it('owner reads its record by id (baseline)', async () => {
      const { status, body } = await getRecord(recA1, tokenA)
      expect(status).toBe(200)
      expect(body.data.__id).toBe(recA1)
      expect(body.data.name).toBe('Alpha')
      expect(body.data.secret).toBe('a-secret')
    })

    it('GET by id from another project is 404 and leaks nothing', async () => {
      const { status, body } = await getRecord(recA1, tokenB)
      expect(status).toBe(404)
      expect(JSON.stringify(body)).not.toContain('a-secret')
    })

    it('PATCH by id from another project is 404 and does not write', async () => {
      const { status } = await apiRaw(`/records/${recA1}`, {
        method: 'PATCH',
        token: tokenB,
        body: { label: LABEL_A, properties: [stringProp('name', 'Hacked')] }
      })
      expect(status).toBe(404)

      const after = await getRecord(recA1, tokenA)
      expect(after.body.data.name).toBe('Alpha')
    })

    it('PUT by id from another project is 404 and does not write', async () => {
      const { status } = await apiRaw(`/records/${recA1}`, {
        method: 'PUT',
        token: tokenB,
        body: { label: LABEL_A, properties: [stringProp('name', 'Hacked')] }
      })
      expect(status).toBe(404)

      const after = await getRecord(recA1, tokenA)
      expect(after.body.data.name).toBe('Alpha')
      expect(after.body.data.secret).toBe('a-secret')
    })

    it('DELETE by id from another project does not delete', async () => {
      // The delete Cypher matches nothing outside the caller's project, so the
      // request no-ops exactly like deleting a nonexistent id.
      await apiRaw(`/records/${recA1}`, { method: 'DELETE', token: tokenB })

      const after = await getRecord(recA1, tokenA)
      expect(after.status).toBe(200)
      expect(after.body.data.secret).toBe('a-secret')
    })

    it('bulk delete by search from another project does not delete', async () => {
      await apiRaw('/records/delete', {
        method: 'POST',
        token: tokenB,
        body: { labels: [LABEL_A] }
      })

      const { body } = await apiRaw('/records/search', {
        method: 'POST',
        token: tokenA,
        body: { labels: [LABEL_A] }
      })
      expect(body.total).toBe(2)
    })

    it('search from a foreign record id returns nothing', async () => {
      const { status, body } = await apiRaw(`/records/${recA1}/search`, {
        method: 'POST',
        token: tokenB,
        body: {}
      })
      expect(status).toBe(200)
      expect(body.total).toBe(0)
      expect(body.data).toHaveLength(0)
    })
  })

  describe('relationships', () => {
    it('attach to a foreign target creates nothing', async () => {
      const before = await relationsCount(tokenB)
      await apiRaw(`/relationships/${recB1}`, {
        method: 'POST',
        token: tokenB,
        body: { targetIds: [recA1] }
      })
      expect(await relationsCount(tokenB)).toBe(before)
      // Project A still has exactly its baseline relationship.
      expect(await relationsCount(tokenA)).toBe(1)
    })

    it('attach from a foreign source creates nothing', async () => {
      await apiRaw(`/relationships/${recA1}`, {
        method: 'POST',
        token: tokenB,
        body: { targetIds: [recB1] }
      })
      expect(await relationsCount(tokenA)).toBe(1)
      expect(await relationsCount(tokenB)).toBe(0)
    })

    it('detach on a foreign source removes nothing', async () => {
      await apiRaw(`/relationships/${recA1}`, {
        method: 'PUT',
        token: tokenB,
        body: { targetIds: [recA2] }
      })
      expect(await relationsCount(tokenA)).toBe(1)
    })
  })

  describe('properties', () => {
    it('owner reads its property by id (baseline)', async () => {
      const { status, body } = await apiRaw(`/properties/${propA}`, { token: tokenA })
      expect(status).toBe(200)
      expect(body.data.name).toBe('secret')
    })

    it('GET property by id from another project is 404', async () => {
      const { status } = await apiRaw(`/properties/${propA}`, { token: tokenB })
      expect(status).toBe(404)
    })

    it('property values from another project leak nothing', async () => {
      const { status, body } = await apiRaw(`/properties/${propA}/values`, {
        method: 'POST',
        token: tokenB,
        body: {}
      })
      expect(status).toBe(200)
      expect(body.data?.values ?? []).toHaveLength(0)
      expect(JSON.stringify(body)).not.toContain('a-secret')
    })

    it('DELETE property from another project does not delete values or the property', async () => {
      // This route is the one place the removed guard was load-bearing: the delete
      // Cypher is now projectId-scoped itself (see PropertyQueryService.matchPart).
      await apiRaw(`/properties/${propA}`, { method: 'DELETE', token: tokenB })

      const prop = await apiRaw(`/properties/${propA}`, { token: tokenA })
      expect(prop.status).toBe(200)
      expect(prop.body.data.name).toBe('secret')

      const record = await getRecord(recA1, tokenA)
      expect(record.body.data.secret).toBe('a-secret')
    })

    it('owner can still delete its own property (baseline for the scoped delete)', async () => {
      await api(`/properties/${propA}`, { method: 'DELETE', token: tokenA })

      const prop = await apiRaw(`/properties/${propA}`, { token: tokenA })
      expect(prop.status).toBe(404)

      const record = await getRecord(recA1, tokenA)
      expect(record.status).toBe(200)
      expect(record.body.data.secret).toBeUndefined()
      // The rest of the record is untouched.
      expect(record.body.data.name).toBe('Alpha')
    })
  })
})
