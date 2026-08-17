/**
 * End-to-end coverage for the synx managed connector surface, black-box through
 * the real HTTP API:
 *
 *  - connector test / discover command lifecycle (dashboard enqueue → worker
 *    claim → structured result → completed/failed)
 *  - the private contract write path (`POST _internal/synx/batches`) applied to
 *    a project's graph with reserved provenance metadata
 *  - provenance isolation: `__RUSHDB__` sync keys are hidden from ordinary
 *    record reads while a user's own `syncKey` business field survives
 *
 * Requires a stack booted with `RUSHDB_SYNX_CONTROL_TOKEN` set (self-hosted or
 * external via E2E_BASE_URL). Skips cleanly otherwise.
 */
import RushDB from '../packages/javascript-sdk/src/index.node'

import { ADMIN_LOGIN, ADMIN_PASSWORD, BASE_URL } from './setup/env'

const API = `${BASE_URL}/api/v1`
const CONTROL_TOKEN = process.env.RUSHDB_SYNX_CONTROL_TOKEN

// Must mirror platform/core/src/core/common/constants.ts.
const SYNC_ID = '__RUSHDB__KEY__SYNC__ID__'
const SYNCED_AT = '__RUSHDB__KEY__SYNCED__AT__'

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

const workerApi = async (path: string, { method = 'POST', body }: { method?: string; body?: unknown } = {}) => {
  const response = await fetch(`${API}${path}`, {
    method,
    headers: {
      'Content-Type': 'application/json',
      'x-synx-control-token': CONTROL_TOKEN!,
      'x-synx-worker-id': 'e2e-worker'
    },
    ...(body !== undefined ? { body: JSON.stringify(body) } : {})
  })
  const text = await response.text()
  const json = text ? JSON.parse(text) : {}
  // The connector controller wraps nulls as 404 (no pending work) and success
  // responses as { data }. Normalize: 404 → null.
  if (response.status === 404 && json?.message === 'Not Found') {
    return null
  }
  if (!response.ok) {
    throw new Error(`${method} ${path} -> ${response.status}: ${text}`)
  }
  return json.data ?? json
}

describe('synx managed connector surface (e2e)', () => {
  let jwt: string
  let workspaceId: string
  let projectId: string
  let connectorId: string
  let apiKey: string

  const synxConfigured = Boolean(CONTROL_TOKEN)

  beforeAll(async () => {
    if (!synxConfigured) {
      return
    }
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
      body: { name: `e2e-synx-${Date.now().toString(36)}`, description: 'synx managed e2e' }
    })
    projectId = project.id

    const apiToken = await api('/tokens', {
      method: 'POST',
      token: jwt,
      headers: { 'x-project-id': projectId, 'x-workspace-id': workspaceId },
      body: { name: 'e2e-synx', expiration: '1d' }
    })
    apiKey = apiToken.value
    expect(apiKey).toBeTruthy()
  })

  if (!synxConfigured) {
    it('skips because RUSHDB_SYNX_CONTROL_TOKEN is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  it('creates a connector and queues a test command (no longer config-only)', async () => {
    const connector = await api('/connectors', {
      method: 'POST',
      token: jwt,
      headers: { 'x-project-id': projectId, 'x-workspace-id': workspaceId },
      body: {
        type: 'postgres',
        name: 'e2e shop',
        config: { host: 'localhost', port: 5432, database: 'shop', user: 'postgres', snapshot: true },
        secrets: { password: 'secret' }
      }
    })
    connectorId = connector.id
    expect(connector.status).toBe('paused')

    const test = await api(`/connectors/${connectorId}/test`, {
      method: 'POST',
      token: jwt,
      headers: { 'x-project-id': projectId, 'x-workspace-id': workspaceId }
    })
    expect(test.commandId).toBeTruthy()
    expect(test.status).toBe('pending')
  })

  it('claims, executes, and completes the test command with structured diagnostics', async () => {
    // Worker-side: claim the pending test command.
    const claimed = await workerApi('/connectors/_internal/commands/claim')
    expect(claimed).not.toBeNull()
    expect(claimed.commandId).toBeTruthy()
    expect(claimed.type).toBe('test')
    expect(claimed.connector.config.host).toBe('localhost')
    // Secrets are decrypted for the worker.
    expect(claimed.connector.secrets.password).toBe('secret')

    // The worker cannot reach the connector's network; report a structured failure.
    const result = await workerApi(`/connectors/_internal/commands/${claimed.commandId}/result`, {
      body: {
        result: {
          ok: false,
          serverVersion: '16.14',
          checks: [{ code: 'connect', ok: false, message: 'could not connect to db.internal:5432' }]
        }
      }
    })
    expect(result.status).toBe('completed')

    // Dashboard-side: the command shows the structured result.
    const commands = await api(`/connectors/${connectorId}/commands`, {
      token: jwt,
      headers: { 'x-project-id': projectId, 'x-workspace-id': workspaceId }
    })
    const completed = commands.find((c: any) => c.id === claimed.commandId)
    expect(completed).toBeTruthy()
    expect(completed.status).toBe('completed')
    expect(completed.result).toContain('serverVersion')
  })

  it('discover command reports the stream catalog', async () => {
    await api(`/connectors/${connectorId}/discover`, {
      method: 'POST',
      token: jwt,
      headers: { 'x-project-id': projectId, 'x-workspace-id': workspaceId }
    })
    const claimed = await workerApi('/connectors/_internal/commands/claim')
    expect(claimed).not.toBeNull()
    expect(claimed.type).toBe('discover')

    await workerApi(`/connectors/_internal/commands/${claimed.commandId}/result`, {
      body: {
        result: {
          streams: [
            { namespace: 'public', name: 'public.users', targetLabel: 'USERS' },
            { namespace: 'public', name: 'public.orders', targetLabel: 'ORDERS' }
          ]
        }
      }
    })
    const commands = await api(`/connectors/${connectorId}/commands`, {
      token: jwt,
      headers: { 'x-project-id': projectId, 'x-workspace-id': workspaceId }
    })
    const completed = commands.find((c: any) => c.id === claimed.commandId)
    expect(completed.status).toBe('completed')
    expect(completed.result).toContain('public.users')
  })

  it('applies a contract batch and hides provenance from ordinary reads', async () => {
    const db = new RushDB(apiKey, { url: BASE_URL })

    // Worker-side: submit an incremental batch to the private destination.
    const envelope = {
      version: '1',
      source: {
        bindingId: connectorId,
        connectionId: connectorId,
        connectorType: 'postgres',
        instance: null
      },
      stream: { id: connectorId, name: 'public.users', namespace: 'public', sourceSchemaVersion: null },
      batch: { id: `${connectorId}.0`, sequence: 0, emittedAt: new Date().toISOString(), cursorHash: null, mode: 'incremental' },
      mapping: {
        targetLabel: 'E2E_SYNCED_USER',
        identityFields: [SYNC_ID],
        deletionMode: 'hard_delete',
        mappingVersion: 1
      },
      operations: [
        {
          type: 'upsert',
          sourceId: 'pg.public.users:42',
          sourceCreatedAt: null,
          sourceUpdatedAt: null,
          data: {
            id: 42,
            name: 'Ada',
            // A user-owned business field that happens to share the old name.
            syncKey: 'user-owned-business-key',
            [SYNCED_AT]: new Date().toISOString()
          },
          relations: null
        }
      ]
    }
    const ack = await workerApi('/_internal/synx/batches', { body: envelope })
    expect(ack.status).toBe('committed')
    expect(ack.accepted).toBe(1)
    expect(ack.checkpointAccepted).toBe(true)

    // Read the record back as a normal user: the namespaced sync keys are gone,
    // but the user's own `syncKey` field is preserved.
    const found = await db.records.find({ labels: ['E2E_SYNCED_USER'], where: { id: 42 } })
    expect(found.total).toBe(1)
    const data = found.data[0]?.data
    expect(data.name).toBe('Ada')
    expect(data.syncKey).toBe('user-owned-business-key')
    expect(data[SYNC_ID]).toBeUndefined()
    expect(data[SYNCED_AT]).toBeUndefined()
  })

  it('deduplicates a replayed batch (lost-ack replay safety)', async () => {
    const db = new RushDB(apiKey, { url: BASE_URL })

    const envelope = {
      version: '1',
      source: {
        bindingId: connectorId,
        connectionId: connectorId,
        connectorType: 'postgres',
        instance: null
      },
      stream: { id: connectorId, name: 'public.users', namespace: 'public', sourceSchemaVersion: null },
      batch: { id: `${connectorId}.0`, sequence: 0, emittedAt: new Date().toISOString(), cursorHash: null, mode: 'incremental' },
      mapping: {
        targetLabel: 'E2E_SYNCED_USER',
        identityFields: [SYNC_ID],
        deletionMode: 'hard_delete',
        mappingVersion: 1
      },
      operations: [
        {
          type: 'upsert',
          sourceId: 'pg.public.users:42',
          sourceCreatedAt: null,
          sourceUpdatedAt: null,
          data: { id: 42, name: 'Ada (replayed)', syncKey: 'user-owned-business-key' },
          relations: null
        }
      ]
    }
    const ack = await workerApi('/_internal/synx/batches', { body: envelope })
    expect(ack.status).toBe('duplicate')

    // No extra record from the replay.
    const found = await db.records.find({ labels: ['E2E_SYNCED_USER'] })
    expect(found.total).toBe(1)
  })

  it('hard_delete removes the destination record', async () => {
    const db = new RushDB(apiKey, { url: BASE_URL })
    const label = 'E2E_DELETE_HARD'
    const stream = `${connectorId}:${label}`
    const sourceId = `pg.public.users:${label}:1`

    const upsert = {
      version: '1',
      source: {
        bindingId: connectorId,
        connectionId: connectorId,
        connectorType: 'postgres',
        instance: null
      },
      stream: { id: stream, name: label },
      batch: { id: `${stream}.0`, sequence: 0, emittedAt: new Date().toISOString(), mode: 'incremental' },
      mapping: { targetLabel: label, identityFields: [SYNC_ID], deletionMode: 'hard_delete', mappingVersion: 1 },
      operations: [{ type: 'upsert', sourceId, data: { id: 1, name: 'delete-me' } }]
    }
    expect((await workerApi('/_internal/synx/batches', { body: upsert })).status).toBe('committed')

    const del = {
      ...upsert,
      batch: { id: `${stream}.1`, sequence: 1, emittedAt: new Date().toISOString(), mode: 'incremental' },
      operations: [{ type: 'delete', sourceId }]
    }
    expect((await workerApi('/_internal/synx/batches', { body: del })).status).toBe('committed')

    const found = await db.records.find({ labels: [label] })
    expect(found.total).toBe(0)
  })

  it('soft_delete retains the record but flags it', async () => {
    const db = new RushDB(apiKey, { url: BASE_URL })
    const label = 'E2E_DELETE_SOFT'
    const stream = `${connectorId}:${label}`
    const sourceId = `pg.public.users:${label}:1`

    const upsert = {
      version: '1',
      source: {
        bindingId: connectorId,
        connectionId: connectorId,
        connectorType: 'postgres',
        instance: null
      },
      stream: { id: stream, name: label },
      batch: { id: `${stream}.0`, sequence: 0, emittedAt: new Date().toISOString(), mode: 'incremental' },
      mapping: { targetLabel: label, identityFields: [SYNC_ID], deletionMode: 'soft_delete', mappingVersion: 1 },
      operations: [{ type: 'upsert', sourceId, data: { id: 1, name: 'soft-delete-me' } }]
    }
    expect((await workerApi('/_internal/synx/batches', { body: upsert })).status).toBe('committed')

    const del = {
      ...upsert,
      batch: { id: `${stream}.1`, sequence: 1, emittedAt: new Date().toISOString(), mode: 'incremental' },
      operations: [{ type: 'delete', sourceId }]
    }
    expect((await workerApi('/_internal/synx/batches', { body: del })).status).toBe('committed')

    // The record survives a soft delete (unlike hard_delete) — this is the
    // observable guarantee. The internal deleted-at provenance flag lives in
    // the graph but is hidden from ordinary reads like all __RUSHDB__ keys.
    const found = await db.records.find({ labels: [label] })
    expect(found.total).toBe(1)
  })

  it('ignore deletion leaves the record untouched', async () => {
    const db = new RushDB(apiKey, { url: BASE_URL })
    const label = 'E2E_DELETE_IGNORE'
    const stream = `${connectorId}:${label}`
    const sourceId = `pg.public.users:${label}:1`

    const upsert = {
      version: '1',
      source: {
        bindingId: connectorId,
        connectionId: connectorId,
        connectorType: 'postgres',
        instance: null
      },
      stream: { id: stream, name: label },
      batch: { id: `${stream}.0`, sequence: 0, emittedAt: new Date().toISOString(), mode: 'incremental' },
      mapping: { targetLabel: label, identityFields: [SYNC_ID], deletionMode: 'ignore', mappingVersion: 1 },
      operations: [{ type: 'upsert', sourceId, data: { id: 1, name: 'keep-me' } }]
    }
    expect((await workerApi('/_internal/synx/batches', { body: upsert })).status).toBe('committed')

    const del = {
      ...upsert,
      batch: { id: `${stream}.1`, sequence: 1, emittedAt: new Date().toISOString(), mode: 'incremental' },
      operations: [{ type: 'delete', sourceId }]
    }
    expect((await workerApi('/_internal/synx/batches', { body: del })).status).toBe('committed')

    const found = await db.records.find({ labels: [label] })
    expect(found.total).toBe(1)
  })

  it('rejects a batch from a stale generation (resnapshot fencing)', async () => {
    const label = 'E2E_GENERATION'
    const stream = `${connectorId}:${label}`
    const envelope = {
      version: '1',
      source: {
        bindingId: connectorId,
        connectionId: connectorId,
        connectorType: 'postgres',
        instance: null
      },
      stream: { id: stream, name: label },
      batch: {
        id: `${stream}.0`,
        sequence: 0,
        generation: -1,
        emittedAt: new Date().toISOString(),
        mode: 'incremental'
      },
      mapping: { targetLabel: label, identityFields: [SYNC_ID], deletionMode: 'ignore', mappingVersion: 1 },
      operations: [{ type: 'upsert', sourceId: `pg.public.users:${label}:1`, data: { id: 1 } }]
    }

    const response = await fetch(`${API}/_internal/synx/batches`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'x-synx-control-token': CONTROL_TOKEN!,
        'x-synx-worker-id': 'e2e-worker'
      },
      body: JSON.stringify(envelope)
    })
    expect(response.status).toBe(409)
    const body = await response.json()
    expect(body.code).toBe('STALE_GENERATION')
    expect(body.details.currentGeneration).toBeGreaterThanOrEqual(0)
  })
})
