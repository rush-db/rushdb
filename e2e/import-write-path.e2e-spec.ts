/**
 * End-to-end coverage for the record import write path (`createMany` / `importJson`),
 * black-box through the real HTTP API with the real JS SDK — the exact path from the
 * managed write-throughput report: batches that previously breached the request budget,
 * `mergeBy` upserts, concurrent batches, and error transparency.
 */
import RushDB from '../packages/javascript-sdk/src/index.node'

import { ADMIN_LOGIN, ADMIN_PASSWORD, BASE_URL } from './setup/env'

const API = `${BASE_URL}/api/v1`
// Unique per run so the suite can be re-run against a persistent external stack.
const RUN = Date.now().toString(36)
const NOTE_TEXT = 'Weir on river left, portage recommended at higher flows. '.repeat(10) // ~570 chars

const api = async (
  path: string,
  {
    method = 'GET',
    token,
    headers = {},
    body
  }: { method?: string; token?: string; headers?: Record<string, string>; body?: unknown } = {}
) => {
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
  if (!response.ok) {
    throw new Error(`${method} ${path} -> ${response.status}: ${JSON.stringify(json)}`)
  }
  return json.data ?? json
}

describe('record import write path (e2e)', () => {
  let db: InstanceType<typeof RushDB>
  let jwt: string
  let workspaceId: string
  let projectId: string

  beforeAll(async () => {
    // Self-hosted boot auto-creates the admin user and Default Workspace.
    const user = await api('/auth/login', {
      method: 'POST',
      body: { login: ADMIN_LOGIN, password: ADMIN_PASSWORD }
    })
    jwt = user.token
    expect(jwt).toBeTruthy()

    const workspaces = await api('/workspaces', { token: jwt })
    workspaceId = workspaces[0]?.id
    expect(workspaceId).toBeTruthy()

    const project = await api('/projects', {
      method: 'POST',
      token: jwt,
      headers: { 'x-workspace-id': workspaceId },
      body: { name: `e2e-import-${RUN}`, description: 'import write path e2e' }
    })
    projectId = project.id
    expect(projectId).toBeTruthy()

    const apiToken = await api('/tokens', {
      method: 'POST',
      token: jwt,
      headers: { 'x-project-id': projectId, 'x-workspace-id': workspaceId },
      body: { name: 'e2e', expiration: '1d' }
    })
    expect(apiToken.value).toBeTruthy()

    db = new RushDB(apiToken.value, { url: BASE_URL })
  })

  it('imports a 150-row note-sized batch without mergeBy (previously breached the request budget)', async () => {
    const label = `E2E_NOTE_${RUN}`
    const rows = Array.from({ length: 150 }, (_, i) => ({
      key: `n-${i}`,
      noteKey: `river-${i % 7}`,
      englishText: NOTE_TEXT
    }))

    const startedAt = Date.now()
    await db.records.createMany({ label, data: rows })
    const elapsed = Date.now() - startedAt

    const found = await db.records.find({ labels: [label], limit: 1 })
    expect(found.total).toBe(150)
    // The reported bug was a ~30s floor per write regardless of size; a small batch on an
    // idle local stack must stay far below the 30s transaction budget.
    expect(elapsed).toBeLessThan(20_000)
  })

  it('upserts with mergeBy without duplicating records', async () => {
    const label = `E2E_RIVER_${RUN}`
    const options = { mergeBy: ['key'], mergeStrategy: 'append' as const }

    const first = Array.from({ length: 100 }, (_, i) => ({
      key: `r-${i}`,
      name: `Section ${i}`
    }))
    const firstStartedAt = Date.now()
    await db.records.createMany({ label, data: first, options })
    const firstElapsed = Date.now() - firstStartedAt

    const afterFirst = await db.records.find({ labels: [label], limit: 1 })
    expect(afterFirst.total).toBe(100)

    // 50 overlapping keys (r-50..r-99) + 50 new (r-100..r-149)
    const second = Array.from({ length: 100 }, (_, i) => ({
      key: `r-${i + 50}`,
      grade: 'III'
    }))
    const secondStartedAt = Date.now()
    await db.records.createMany({ label, data: second, options })
    const secondElapsed = Date.now() - secondStartedAt
    // The reported bug was ~0.45s/merged row driven by an unindexed, project-wide (not
    // label-scoped) match — a 100-row batch took 40+ seconds. Scoping the match to this
    // label's node set must keep a 100-row merge on an idle local stack far below that.
    expect(firstElapsed).toBeLessThan(20_000)
    expect(secondElapsed).toBeLessThan(20_000)

    const afterSecond = await db.records.find({ labels: [label], limit: 1 })
    expect(afterSecond.total).toBe(150)

    // The overlapping record was matched, not duplicated, and append kept both property sets.
    const merged = await db.records.find({ labels: [label], where: { key: 'r-75' } })
    expect(merged.total).toBe(1)
    expect(merged.data[0].data).toMatchObject({ key: 'r-75', name: 'Section 75', grade: 'III' })

    // Re-importing identical rows stays idempotent.
    await db.records.createMany({ label, data: second, options })
    const afterThird = await db.records.find({ labels: [label], limit: 1 })
    expect(afterThird.total).toBe(150)
  })

  it('handles mergeBy keys that need identifier quoting (spaces, backticks)', async () => {
    const label = `E2E_ODDKEY_${RUN}`
    const options = { mergeBy: ['section key', 'weird`name'], mergeStrategy: 'append' as const }
    const rows = Array.from({ length: 20 }, (_, i) => ({
      'section key': `s-${i}`,
      'weird`name': `w-${i}`
    }))

    await db.records.createMany({ label, data: rows, options })
    await db.records.createMany({ label, data: rows, options })

    const found = await db.records.find({ labels: [label], limit: 1 })
    expect(found.total).toBe(20)
  })

  it('serves concurrent createMany batches to distinct labels', async () => {
    const labels = Array.from({ length: 4 }, (_, i) => `E2E_CONC_${i}_${RUN}`)
    const rows = Array.from({ length: 100 }, (_, i) => ({
      key: `c-${i}`,
      englishText: NOTE_TEXT
    }))

    // The report observed every concurrent batch failing with an opaque 400.
    await Promise.all(labels.map((label) => db.records.createMany({ label, data: rows })))

    for (const label of labels) {
      const found = await db.records.find({ labels: [label], limit: 1 })
      expect(found.total).toBe(100)
    }
  })

  it('runs side effects (project stats recount) after a tx-wrapped import commits', async () => {
    const label = `E2E_TXSTATS_${RUN}`
    const rows = Array.from({ length: 25 }, (_, i) => ({ key: `tx-${i}`, name: `Row ${i}` }))

    const statsRecordCount = async (): Promise<number> => {
      const project = await api(`/projects/${projectId}`, {
        token: jwt,
        headers: { 'x-project-id': projectId, 'x-workspace-id': workspaceId }
      })
      try {
        return JSON.parse(project.stats ?? '{}').records ?? 0
      } catch {
        return 0
      }
    }

    // Writes inside a user-defined transaction are invisible to other transactions until
    // commit — the side-effect runner must defer to the commit, not count mid-transaction
    // (the original bug recounted 0 records for a fresh tx-wrapped import).
    const tx = await db.tx.begin({ ttl: 30_000 })
    await db.records.createMany({ label, data: rows }, tx)
    await db.tx.commit(tx)

    // Every record in the project is committed at this point, so the stats counter must
    // converge on the project-wide total (recounts from earlier tests can only land on
    // smaller totals). The recount runs post-response in the background; poll briefly.
    const expectedTotal = (await db.records.find({ limit: 1 })).total ?? 0
    expect(expectedTotal).toBeGreaterThanOrEqual(rows.length)

    const deadline = Date.now() + 30_000
    let counted = 0
    while (Date.now() < deadline) {
      counted = await statsRecordCount()
      if (counted >= expectedTotal) {
        break
      }
      await new Promise((r) => setTimeout(r, 1000))
    }
    expect(counted).toBeGreaterThanOrEqual(expectedTotal)
  })

  it('surfaces the server error cause through the SDK instead of a bare status code', async () => {
    // A text payload that is neither JSON nor JSONL passes SDK-side validation but is
    // rejected by the import endpoint with a 400; the SDK must carry that message through
    // instead of throwing a bare `Error("400")`.
    await expect(
      db.records.importJson({ label: `E2E_BADJSON_${RUN}`, data: '{{ definitely not json' })
    ).rejects.toThrow(/400 .*(import failed|invalid)/i)
  })
})
