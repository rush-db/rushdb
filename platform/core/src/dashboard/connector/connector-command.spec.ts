import { ForbiddenException, NotFoundException } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'

import { EntityService } from '@/core/entity/entity.service'
import { ConnectorSecretService } from '@/dashboard/connector/connector-secret.service'
import { ConnectorService, deriveConnectorHealth } from '@/dashboard/connector/connector.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

import type { ConnectorRepository } from '@/dashboard/connector/model/connector.repository'
import type { ProjectService } from '@/dashboard/project/project.service'
import type { TokenService } from '@/dashboard/token/token.service'

describe('ConnectorService command lifecycle', () => {
  const row = {
    id: 'conn-1',
    projectId: 'proj-1',
    name: 'shop pg',
    type: 'postgres',
    config: JSON.stringify({ host: 'db.internal', database: 'shop' }),
    transform: JSON.stringify({}),
    status: 'paused',
    lastError: null,
    lagMs: null,
    stats: null,
    createdBy: null,
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-01-01T00:00:00.000Z'
  }

  const command = {
    id: 'cmd-1',
    connectorId: 'conn-1',
    projectId: 'proj-1',
    type: 'test',
    status: 'pending',
    payload: null,
    result: null,
    errorMessage: null,
    requestedBy: 'user-1',
    claimedBy: null,
    createdAt: '2026-01-01T00:00:00.000Z',
    claimedAt: null,
    completedAt: null
  }

  function repo(overrides: Partial<Record<keyof ConnectorRepository, unknown>> = {}): ConnectorRepository {
    const base = {
      findById: jest.fn(),
      findByProjectId: jest.fn(),
      findSecret: jest.fn(),
      findOffsets: jest.fn(),
      findEvents: jest.fn(),
      findLease: jest.fn(),
      findPendingCommand: jest.fn(),
      findCommands: jest.fn(),
      createCommand: jest.fn(),
      claimCommand: jest.fn(),
      completeCommand: jest.fn(),
      findCommandById: jest.fn(),
      addEvent: jest.fn(),
      create: jest.fn(),
      update: jest.fn(),
      delete: jest.fn(),
      deleteLease: jest.fn(),
      upsertLease: jest.fn(),
      upsertOffset: jest.fn(),
      upsertSecret: jest.fn(),
      deleteExpiredLeases: jest.fn(),
      findRunnableWithoutLease: jest.fn(),
      claimNextRunnable: jest.fn(),
      renewLeaseIfOwned: jest.fn(),
      createRun: jest.fn(),
      findRunById: jest.fn(),
      findRuns: jest.fn(),
      findActiveRuns: jest.fn(async () => []),
      updateRun: jest.fn(),
      findConnectorDefinition: jest.fn(async () => undefined),
      listConnectorDefinitions: jest.fn(async () => [])
    } as unknown as Record<keyof ConnectorRepository, unknown>
    return { ...base, ...overrides } as unknown as ConnectorRepository
  }

  const config = { get: jest.fn(() => 'control-token') } as unknown as ConfigService

  function service(repository: ConnectorRepository): ConnectorService {
    return new ConnectorService(
      repository,
      new ConnectorSecretService(config),
      {} as ProjectService,
      {} as TokenService,
      config,
      {} as EntityService,
      {} as NeogmaService,
      {} as import('@/dashboard/synx/synx.connectors').SynxConnectorRegistry,
      {} as import('@/dashboard/workspace/workspace.service').WorkspaceService
    )
  }

  beforeEach(() => jest.clearAllMocks())

  it('queues a test command instead of only validating config', async () => {
    const repository = repo({
      findById: jest.fn(async () => row),
      createCommand: jest.fn(async (d: object) => ({ ...command, ...(d as object) }))
    })
    const result = await service(repository).test('conn-1', 'proj-1', 'user-1')
    expect(typeof result.commandId).toBe('string')
    expect(result.commandId.length).toBeGreaterThan(0)
    expect(result.status).toBe('pending')
    expect(repository.createCommand).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'test', status: 'pending', connectorId: 'conn-1' })
    )
  })

  it('claims the oldest pending command and returns decrypted secrets', async () => {
    const repository = repo({
      findPendingCommand: jest.fn(async () => [command]),
      claimCommand: jest.fn(async (id: string) => ({ ...command, status: 'claimed', claimedBy: 'worker-1' })),
      findById: jest.fn(async () => row),
      findSecret: jest.fn(async () => null)
    })
    const result = await service(repository).claimCommand('worker-1', 'control-token')
    expect(result).toMatchObject({
      commandId: 'cmd-1',
      type: 'test',
      connectorId: 'conn-1',
      connector: { name: 'shop pg', type: 'postgres' }
    })
    expect(repository.claimCommand).toHaveBeenCalledWith('cmd-1', 'worker-1')
  })

  it('completes a claimed command and records an event', async () => {
    const repository = repo({
      findCommandById: jest.fn(async () => ({ ...command, status: 'claimed' })),
      completeCommand: jest.fn(async (id: string, data: object) => ({ ...command, ...data })),
      findById: jest.fn(async () => row),
      addEvent: jest.fn(async () => ({}) as never)
    })
    const result = await service(repository).completeCommand(
      'cmd-1',
      { ok: true },
      undefined,
      'control-token'
    )
    expect(result.status).toBe('completed')
    expect(repository.completeCommand).toHaveBeenCalledWith(
      'cmd-1',
      expect.objectContaining({ status: 'completed', result: JSON.stringify({ ok: true }) })
    )
    expect(repository.addEvent).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'command', message: 'Command test completed' })
    )
  })

  it('rejects a control-token mismatch', async () => {
    const badConfig = { get: jest.fn(() => 'expected') } as unknown as ConfigService
    const svc = new ConnectorService(
      repo(),
      new ConnectorSecretService(badConfig),
      {} as ProjectService,
      {} as TokenService,
      badConfig,
      {} as EntityService,
      {} as NeogmaService,
      {} as import('@/dashboard/synx/synx.connectors').SynxConnectorRegistry,
      {} as import('@/dashboard/workspace/workspace.service').WorkspaceService
    )
    await expect(svc.claimCommand('worker-1', 'wrong')).rejects.toBeInstanceOf(ForbiddenException)
  })

  it('rejects completing a command that was never claimed', async () => {
    const repository = repo({
      findCommandById: jest.fn(async () => ({ ...command, status: 'pending' }))
    })
    await expect(
      service(repository).completeCommand('cmd-1', {}, undefined, 'control-token')
    ).rejects.toThrow('not in a claimable state')
  })

  it('returns null when no command is pending', async () => {
    const repository = repo({ findPendingCommand: jest.fn(async () => []) })
    expect(await service(repository).claimCommand('worker-1', 'control-token')).toBeNull()
  })

  it('rejects unknown commands in getCommand', async () => {
    const repository = repo({ findCommandById: jest.fn(async () => undefined) })
    await expect(service(repository).getCommand('nope', 'proj-1')).rejects.toBeInstanceOf(NotFoundException)
  })

  it('delete with deleteRecords deletes synced records by label, then the connector', async () => {
    const mongoRow = {
      ...row,
      type: 'mongodb',
      config: JSON.stringify({ host: 'db.internal', database: 'app', collections: ['customers', 'orders'] })
    }
    const deleteEntity = jest.fn(async (_p: { searchQuery: { labels?: string[] } }) => ({
      message: 'deleted'
    }))
    const commit = jest.fn(async () => ({}))
    const rollback = jest.fn(async () => ({}))
    const close = jest.fn(async () => ({}))
    const beginTransaction = jest.fn(() => ({ commit, rollback }))
    const session = { beginTransaction, close }
    const driver = { session: jest.fn(() => session) }
    const neogma = { getDriver: jest.fn(() => driver) } as unknown as NeogmaService
    const entity = { delete: deleteEntity } as unknown as EntityService

    const repository = repo({
      findById: jest.fn(async () => mongoRow),
      delete: jest.fn(async () => true),
      addEvent: jest.fn(async () => ({}) as never)
    })
    const svc = new ConnectorService(
      repository,
      new ConnectorSecretService(config),
      {} as ProjectService,
      {} as TokenService,
      config,
      entity,
      neogma,
      {} as import('@/dashboard/synx/synx.connectors').SynxConnectorRegistry,
      {} as import('@/dashboard/workspace/workspace.service').WorkspaceService
    )

    const result = await svc.delete('conn-1', 'proj-1', true)
    expect(result).toBe(true)
    // One delete per derived label (CUSTOMERS, ORDERS).
    expect(deleteEntity).toHaveBeenCalledTimes(2)
    const labels = deleteEntity.mock.calls.map((c) => c[0]?.searchQuery?.labels?.[0])
    expect(labels.sort()).toEqual(['CUSTOMERS', 'ORDERS'])
    expect(commit).toHaveBeenCalled()
    expect(repository.delete).toHaveBeenCalledWith('conn-1')
  })

  it('delete without deleteRecords only removes the connector', async () => {
    const repository = repo({
      findById: jest.fn(async () => row),
      delete: jest.fn(async () => true),
      addEvent: jest.fn(async () => ({}) as never)
    })
    const svc = new ConnectorService(
      repository,
      new ConnectorSecretService(config),
      {} as ProjectService,
      {} as TokenService,
      config,
      {} as EntityService,
      {} as NeogmaService,
      {} as import('@/dashboard/synx/synx.connectors').SynxConnectorRegistry,
      {} as import('@/dashboard/workspace/workspace.service').WorkspaceService
    )
    await svc.delete('conn-1', 'proj-1', false)
    expect(repository.delete).toHaveBeenCalledWith('conn-1')
  })

  it('claim uses the atomic claim and returns the claimed connector', async () => {
    const repository = repo({
      claimNextRunnable: jest.fn(async () => 'conn-1'),
      findById: jest.fn(async () => row),
      findSecret: jest.fn(async () => null),
      findOffsets: jest.fn(async () => []),
      createRun: jest.fn(async (d: object) => ({ id: 'run-1', ...(d as object) })),
      update: jest.fn(async () => row)
    })
    const svc = service(repository)
    const result = await svc.claim('worker-1', 'control-token')
    expect(result).not.toBeNull()
    expect(result?.connectorId).toBe('conn-1')
    expect(repository.claimNextRunnable).toHaveBeenCalledWith(
      'worker-1',
      expect.any(String),
      expect.any(String)
    )
  })

  it('claim returns null when nothing is available to claim', async () => {
    const repository = repo({ claimNextRunnable: jest.fn(async () => undefined) })
    expect(await service(repository).claim('worker-1', 'control-token')).toBeNull()
  })

  it('heartbeat renews the lease with a single owned check and reports lease_lost when stale', async () => {
    const repository = repo({
      findById: jest.fn(async () => ({ ...row, status: 'running' })),
      renewLeaseIfOwned: jest.fn(async () => false)
    })
    const result = await service(repository).heartbeat(
      'conn-1',
      'worker-1',
      { leaseTtlMs: 15000 },
      'control-token'
    )
    expect(result).toMatchObject({ active: false, reason: 'lease_lost' })
    // The stale worker must never extend a lease it lost.
    expect(repository.renewLeaseIfOwned).toHaveBeenCalledWith(
      'conn-1',
      'worker-1',
      expect.any(String),
      expect.any(String)
    )
  })

  it('heartbeat returns active when the lease renew succeeds', async () => {
    const repository = repo({
      findById: jest.fn(async () => ({ ...row, status: 'running' })),
      renewLeaseIfOwned: jest.fn(async () => true)
    })
    const result = await service(repository).heartbeat(
      'conn-1',
      'worker-1',
      { leaseTtlMs: 15000 },
      'control-token'
    )
    expect(result).toMatchObject({ active: true, status: 'running' })
  })

  it('a run marked failed by status(error) survives a later release (offset-failure replay safety)', async () => {
    // A fatal offset-commit failure halts the pipeline; the worker reports
    // status=error first (marking the active run failed with the reason), then
    // releases the lease. Release must NOT clobber that failed run back to
    // "stopped" — the run history must keep the failure.
    const repository = repo({
      findById: jest.fn(async () => ({ ...row, status: 'error' })),
      update: jest.fn(async () => row),
      findActiveRuns: jest.fn(async () => []),
      findLease: jest.fn(async () => ({ connectorId: 'conn-1', workerId: 'worker-1' })),
      deleteLease: jest.fn(async () => {})
    })

    // status=error finishes the active run as failed...
    await service(repository).updateStatus(
      'conn-1',
      { status: 'error', lastError: 'offset commit failed' },
      'control-token'
    )
    // ...then release only finishes runs still marked running — the failed run
    // is not active, so it must be left untouched.
    await service(repository).release('conn-1', 'worker-1', 'control-token')

    expect(repository.deleteLease).toHaveBeenCalled()
    // findActiveRuns was consulted on release and found nothing (the failed run
    // is excluded), so updateRun was never called for a "stopped" transition.
    const stopFinishes = (repository.updateRun as jest.Mock).mock.calls.filter(
      (call) => call[1]?.status === 'stopped'
    )
    expect(stopFinishes).toHaveLength(0)
  })

  describe('deriveConnectorHealth', () => {
    it('is healthy for a running connector with no lag', () => {
      const health = deriveConnectorHealth({ status: 'running', lagMs: null, stats: null })
      expect(health.score).toBe(100)
      expect(health.level).toBe('healthy')
      expect(health.reasons).toHaveLength(0)
    })

    it('is critical for an errored connector', () => {
      const health = deriveConnectorHealth({ status: 'error', lagMs: 5, stats: null })
      expect(health.score).toBe(0)
      expect(health.level).toBe('critical')
      expect(health.reasons).toContain('status=error')
    })

    it('degrades with high lag', () => {
      const health = deriveConnectorHealth({ status: 'running', lagMs: 90_000, stats: null })
      expect(health.score).toBeLessThanOrEqual(40)
      expect(health.level).toBe('critical')
      expect(health.reasons.join(' ')).toContain('lag')
    })

    it('drops score for failed batches in stats', () => {
      const health = deriveConnectorHealth({
        status: 'running',
        lagMs: null,
        stats: JSON.stringify({ pipelines: { a: { batchesFailed: 3 }, b: { batchesFailed: 0 } } })
      })
      expect(health.score).toBe(75)
      expect(health.reasons.join(' ')).toContain('3 failed batch')
    })
  })

  describe('catalog', () => {
    const full = {
      connectors: [
        { id: 'postgres', entitlement: 'free' },
        { id: 'mysql', entitlement: 'paid' },
        { id: 'hubspot', entitlement: 'top_tier' }
      ],
      unavailable: []
    }

    it('returns the full catalog in self-hosted mode (no plan gating)', async () => {
      const configSvc = {
        get: jest.fn((k: string) => (k === 'RUSHDB_SELF_HOSTED' ? 'true' : 'control-token'))
      }
      const s = new ConnectorService(
        repo({}),
        new ConnectorSecretService(configSvc as unknown as ConfigService),
        {} as ProjectService,
        {} as TokenService,
        configSvc as unknown as ConfigService,
        {} as EntityService,
        {} as NeogmaService,
        {
          list: () => full,
          listForPlan: () => full
        } as unknown as import('@/dashboard/synx/synx.connectors').SynxConnectorRegistry,
        {} as import('@/dashboard/workspace/workspace.service').WorkspaceService
      )
      const catalog = await s.catalog('ws-1')
      expect(catalog.connectors).toHaveLength(3)
    })

    it('forwards the workspace plan to the synx registry (not hardcoded)', async () => {
      const configSvc = {
        get: jest.fn((k: string) => (k === 'RUSHDB_SELF_HOSTED' ? 'false' : 'control-token'))
      }
      const listForPlan = jest.fn(() => ({ connectors: [{ id: 'postgres' }], unavailable: [] }))
      const s = new ConnectorService(
        repo({}),
        new ConnectorSecretService(configSvc as unknown as ConfigService),
        {} as ProjectService,
        {} as TokenService,
        configSvc as unknown as ConfigService,
        {} as EntityService,
        {} as NeogmaService,
        {
          list: () => full,
          listForPlan
        } as unknown as import('@/dashboard/synx/synx.connectors').SynxConnectorRegistry,
        {
          getWorkspace: jest.fn(async () => ({ getProperties: () => ({ planId: 'pro' }) }))
        } as unknown as import('@/dashboard/workspace/workspace.service').WorkspaceService
      )
      const catalog = await s.catalog('ws-1')
      expect(listForPlan).toHaveBeenCalledWith('pro')
      expect(catalog.connectors).toEqual([{ id: 'postgres' }])
    })
  })

  describe('spec-backed connector types (no hardcoded union)', () => {
    function svcWith(isRegistered: boolean) {
      const repository = repo({
        findById: jest.fn(async () => row),
        create: jest.fn(async (d: object) => ({ ...row, ...(d as object) })),
        upsertSecret: jest.fn(async () => undefined),
        findSecret: jest.fn(async () => null)
      })
      const s = new ConnectorService(
        repository,
        new ConnectorSecretService(config),
        { getProject: jest.fn(async () => ({})) } as unknown as ProjectService,
        {} as TokenService,
        config,
        {} as EntityService,
        {} as NeogmaService,
        {
          isRegistered: jest.fn(async () => isRegistered)
        } as unknown as import('@/dashboard/synx/synx.connectors').SynxConnectorRegistry,
        {} as import('@/dashboard/workspace/workspace.service').WorkspaceService
      )
      return s
    }

    it('accepts a spec type that is registered (e.g. hubspot)', async () => {
      const s = svcWith(true)
      const created = await s.create(
        {
          name: 'hubspot',
          type: 'hubspot' as never,
          config: { baseUrl: 'https://api.hubapi.com' },
          secrets: { accessToken: 'tok' }
        },
        'proj-1',
        'user-1'
      )
      expect(created.id).toBeTruthy()
    })

    it('rejects a spec type that is not registered', async () => {
      const s = svcWith(false)
      await expect(
        s.create({ name: 'ghost', type: 'ghost' as never, config: {} }, 'proj-1', 'user-1')
      ).rejects.toThrow('Unsupported connector type')
    })
  })
})
