import {
  BadRequestException,
  ForbiddenException,
  Injectable,
  NotFoundException,
  ServiceUnavailableException
} from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { uuidv7 } from 'uuidv7'

import { getCurrentISO } from '@/common/utils/getCurrentISO'
import { toBoolean } from '@/common/utils/toBolean'
import { EntityService } from '@/core/entity/entity.service'
import { ConnectorSecretService } from '@/dashboard/connector/connector-secret.service'
import {
  CONNECTOR_COMMAND_TYPES,
  CONNECTOR_STATUSES,
  CONNECTOR_TYPES,
  ConnectorCommandType,
  ConnectorStatus,
  ConnectorTransform,
  ConnectorType
} from '@/dashboard/connector/connector.types'
import { CreateConnectorDto, UpdateConnectorDto } from '@/dashboard/connector/dto/create-connector.dto'
import {
  ConnectorHeartbeatDto,
  ConnectorOffsetDto,
  ConnectorStatusDto
} from '@/dashboard/connector/dto/worker-connector.dto'
import { ConnectorRepository } from '@/dashboard/connector/model/connector.repository'
import { ProjectService } from '@/dashboard/project/project.service'
import { SynxConnectorRegistry } from '@/dashboard/synx/synx.connectors'
import { TokenService } from '@/dashboard/token/token.service'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import { NeogmaService } from '@/database/neogma/neogma.service'
import { DEFAULT_TRANSACTION_TIMEOUT_MS } from '@/database/transaction.constants'

import type { ConnectorCommandRow, ConnectorEventRow, ConnectorRow } from '@/database/sql/schema/types'

const INTERNAL_DESTINATION_API_KEY = '__rushdbDestinationApiKey'

export interface ConnectorHealth {
  score: number
  level: 'healthy' | 'degraded' | 'critical'
  reasons: string[]
}

/**
 * Derive a 0–100 health score from the connector's current row state. The
 * score is a lightweight operational signal, not an SLA: status is the primary
 * factor, then lag (stale change stream = degradation), then rejection
 * pressure when stats carry a rejection counter.
 */
export function deriveConnectorHealth(
  row: Pick<ConnectorRow, 'status' | 'lagMs' | 'stats'>
): ConnectorHealth {
  const reasons: string[] = []
  let score = 100

  if (row.status === 'error') {
    score = 0
    reasons.push('status=error')
  } else if (row.status === 'paused') {
    score = 50
    reasons.push('status=paused')
  }

  if (row.status !== 'error') {
    const lag = row.lagMs
    if (typeof lag === 'number' && lag > 0) {
      if (lag > 60_000) {
        score = Math.min(score, 40)
        reasons.push(`lag ${Math.round(lag / 1000)}s`)
      } else if (lag > 10_000) {
        score = Math.min(score, 75)
        reasons.push(`lag ${Math.round(lag / 1000)}s`)
      }
    }

    let rejected = 0
    try {
      const stats = row.stats ? JSON.parse(row.stats) : null
      const pipelines = stats?.pipelines as Record<string, { batchesFailed?: number }> | undefined
      if (pipelines) {
        rejected = Object.values(pipelines).reduce((sum, p) => sum + Number(p?.batchesFailed ?? 0), 0)
      }
    } catch {
      /* unparseable stats are not a health signal */
    }
    if (rejected > 0) {
      score = Math.max(0, score - 25)
      reasons.push(`${rejected} failed batch(es)`)
    }
  }

  const level: ConnectorHealth['level'] =
    score >= 80 ? 'healthy'
    : score >= 45 ? 'degraded'
    : 'critical'
  return { score, level, reasons }
}

type PublicConnector = Omit<ConnectorRow, 'config' | 'transform' | 'stats'> & {
  config: Record<string, unknown>
  transform: ConnectorTransform
  stats?: Record<string, unknown>
  secrets: Record<string, '••••'>
  /** 0–100 health score derived from status, lag, and rejection pressure. */
  health: ConnectorHealth
}

@Injectable()
export class ConnectorService {
  constructor(
    private readonly connectorRepository: ConnectorRepository,
    private readonly connectorSecretService: ConnectorSecretService,
    private readonly projectService: ProjectService,
    private readonly tokenService: TokenService,
    private readonly configService: ConfigService,
    private readonly entityService: EntityService,
    private readonly neogmaService: NeogmaService,
    private readonly connectorRegistry: SynxConnectorRegistry,
    private readonly workspaceService: WorkspaceService
  ) {}

  async create(dto: CreateConnectorDto, projectId: string, createdBy?: string): Promise<PublicConnector> {
    this.assertSynxEnabled()
    await this.projectService.getProject(projectId)
    await this.assertValidType(dto.type)
    this.validateConfig(dto.type, dto.config)

    const now = getCurrentISO()
    const row = await this.connectorRepository.create({
      id: uuidv7(),
      projectId,
      name: dto.name,
      type: dto.type,
      config: JSON.stringify(dto.config),
      transform: JSON.stringify(this.normalizeTransform(dto.transform)),
      status: 'paused',
      createdBy,
      createdAt: now,
      updatedAt: now
    })

    if (dto.secrets) {
      await this.storeSecrets(row.id, dto.secrets)
    }
    await this.event(row, 'created', 'Connector created')
    return this.toPublic(row)
  }

  async list(projectId: string): Promise<PublicConnector[]> {
    this.assertSynxEnabled()
    const rows = await this.connectorRepository.findByProjectId(projectId)
    return rows.map((row) => this.toPublic(row))
  }

  /**
   * Provider catalog for a workspace, partitioned by its plan. The connector
   * union and per-connector minimum tiers live in the synx provider registry —
   * this service only resolves the workspace plan and forwards it.
   *
   * Self-hosted deployments have no billing/plan gating, so every provider is
   * available.
   */
  async catalog(workspaceId?: string) {
    this.assertSynxEnabled()
    const selfHosted = this.configService.get('RUSHDB_SELF_HOSTED')
    if (toBoolean(selfHosted) || !workspaceId) {
      return this.connectorRegistry.list()
    }
    const planId = (await this.workspaceService.getWorkspace(workspaceId))?.getProperties()?.planId
    return this.connectorRegistry.listForPlan(planId)
  }

  async get(id: string, projectId: string): Promise<PublicConnector> {
    this.assertSynxEnabled()
    return this.toPublic(await this.getOwned(id, projectId))
  }

  async update(id: string, projectId: string, dto: UpdateConnectorDto): Promise<PublicConnector> {
    this.assertSynxEnabled()
    const current = await this.getOwned(id, projectId)
    const now = getCurrentISO()
    const patch: Partial<ConnectorRow> = { updatedAt: now }

    if (dto.name) {
      patch.name = dto.name
    }
    if (dto.config) {
      this.validateConfig(current.type as ConnectorType, dto.config)
      patch.config = JSON.stringify(dto.config)
    }
    if (dto.transform) {
      patch.transform = JSON.stringify(this.normalizeTransform(dto.transform))
    }

    if (dto.secrets) {
      await this.storeSecrets(id, dto.secrets)
    }

    const row = await this.connectorRepository.update(id, patch)
    await this.event(row, 'updated', 'Connector updated')
    return this.toPublic(row)
  }

  async delete(id: string, projectId: string, deleteRecords = false): Promise<boolean> {
    this.assertSynxEnabled()
    const row = await this.getOwned(id, projectId)
    if (deleteRecords) {
      await this.deleteSyncedRecords(row)
    }
    await this.event(
      row,
      'deleted',
      deleteRecords ? 'Connector and its records deleted' : 'Connector deleted'
    )
    return this.connectorRepository.delete(id)
  }

  /**
   * Deletes the records a connector synced into this project. Synced records
   * are identified by their target labels, which the worker derives from the
   * configured collections/tables (uppercased). Runs on its own transaction so
   * a large backfill does not hold the request's transaction budget.
   */
  private async deleteSyncedRecords(row: ConnectorRow): Promise<void> {
    const config = this.parse(row.config) as Record<string, unknown>
    const entities =
      Array.isArray(config.collections) ? (config.collections as string[])
      : Array.isArray(config.tables) ? (config.tables as string[])
      : []
    if (entities.length === 0) {
      return
    }
    const labels = entities.map((entity) => entity.split('.').pop()!.toUpperCase())

    const session = this.neogmaService.getDriver().session()
    const transaction = session.beginTransaction({ timeout: DEFAULT_TRANSACTION_TIMEOUT_MS })
    try {
      for (const label of labels) {
        await this.entityService.delete({
          projectId: row.projectId,
          transaction,
          searchQuery: { labels: [label] }
        })
      }
      await transaction.commit()
    } catch (error) {
      await transaction?.rollback?.().catch(() => undefined)
      throw error
    } finally {
      await session.close()
    }
  }

  async setLifecycle(id: string, projectId: string, status: Extract<ConnectorStatus, 'paused' | 'running'>) {
    this.assertSynxEnabled()
    const row = await this.getOwned(id, projectId)
    if (status === 'paused') {
      await this.connectorRepository.deleteLease(id)
    }
    const updated = await this.connectorRepository.update(id, {
      status,
      lastError: null,
      updatedAt: getCurrentISO()
    })
    await this.event(updated, status === 'running' ? 'resumed' : 'paused', `Connector ${status}`)
    return this.toPublic(updated)
  }

  async resnapshot(id: string, projectId: string) {
    this.assertSynxEnabled()
    const row = await this.getOwned(id, projectId)
    // A real resnapshot: bump the generation and drop the snapshot +
    // change-stream offsets so the worker re-runs the backfill and captures a
    // fresh change-stream boundary when it next claims the connector. Without
    // this, the source skips the snapshot (its `snapshot:<collection>` offset
    // already exists) and only streams new changes.
    //
    // The generation is a fencing token: batches submitted by a worker still
    // holding a pre-resnapshot lease carry an older generation and are
    // rejected by the destination, so a stale worker can never write new
    // snapshot data into the graph after a resnapshot has invalidated it.
    const generation = (row.generation ?? 0) + 1
    await this.connectorRepository.update(id, { generation, updatedAt: getCurrentISO() })
    await this.connectorRepository.deleteOffsetsByPrefix(id, 'snapshot:')
    await this.connectorRepository.deleteOffsetsByPrefix(id, 'cs:')
    await this.connectorRepository.deleteOffsetsByPrefix(id, 'synx:')
    await this.connectorRepository.deleteLease(id)
    await this.event(
      row,
      'resnapshot_requested',
      `Connector resnapshot requested (generation ${generation}, offsets + lease cleared)`
    )
    return this.toPublic({ ...row, generation })
  }

  async test(id: string, projectId: string, requestedBy?: string) {
    this.assertSynxEnabled()
    const row = await this.getOwned(id, projectId)
    this.validateConfig(row.type as ConnectorType, this.parse(row.config))
    const command = await this.enqueueCommand(row, 'test', requestedBy)
    return {
      ok: true,
      connectorId: id,
      commandId: command.id,
      status: command.status,
      message: 'Live test queued for a synx worker.'
    }
  }

  async discover(id: string, projectId: string, requestedBy?: string) {
    this.assertSynxEnabled()
    const row = await this.getOwned(id, projectId)
    this.validateConfig(row.type as ConnectorType, this.parse(row.config))
    const command = await this.enqueueCommand(row, 'discover', requestedBy)
    return {
      ok: true,
      connectorId: id,
      commandId: command.id,
      status: command.status,
      message: 'Discovery queued for a synx worker.'
    }
  }

  async databases(id: string, projectId: string, requestedBy?: string) {
    this.assertSynxEnabled()
    const row = await this.getOwned(id, projectId)
    this.validateConfig(row.type as ConnectorType, this.parse(row.config))
    const command = await this.enqueueCommand(row, 'databases', requestedBy)
    return {
      ok: true,
      connectorId: id,
      commandId: command.id,
      status: command.status,
      message: 'Database listing queued for a synx worker.'
    }
  }

  /**
   * Queue a durable `start`: resume a paused connector's run as a claimed
   * command so the worker can surface the run/connection state it validated.
   */
  async start(id: string, projectId: string, requestedBy?: string) {
    this.assertSynxEnabled()
    const row = await this.getOwned(id, projectId)
    this.validateConfig(row.type as ConnectorType, this.parse(row.config))
    const command = await this.enqueueCommand(row, 'start', requestedBy)
    return {
      ok: true,
      connectorId: id,
      commandId: command.id,
      status: command.status,
      message: 'Start queued for a synx worker.'
    }
  }

  /**
   * Queue a durable `replay`: rewind the stream checkpoints to the last
   * committed boundary so the worker re-delivers post-checkpoint operations
   * without a full resnapshot. The source stays at its snapshot; only the
   * change-stream/sequence checkpoint is reset, so already-committed records
   * dedupe and only the missing tail is re-applied.
   */
  async replay(id: string, projectId: string, requestedBy?: string) {
    this.assertSynxEnabled()
    const row = await this.getOwned(id, projectId)
    this.validateConfig(row.type as ConnectorType, this.parse(row.config))
    // Rewind change-stream + sequence checkpoints but keep the snapshot so no
    // full backfill is triggered.
    await this.connectorRepository.deleteOffsetsByPrefix(id, 'cs:')
    await this.connectorRepository.deleteOffsetsByPrefix(id, 'synx:')
    await this.connectorRepository.deleteLease(id)
    const command = await this.enqueueCommand(row, 'replay', requestedBy)
    await this.event(
      row,
      'replay_requested',
      'Replay queued: change-stream + sequence offsets rewound, snapshot kept'
    )
    return {
      ok: true,
      connectorId: id,
      commandId: command.id,
      status: command.status,
      message: 'Replay queued for a synx worker.'
    }
  }

  /** Queue a durable `cancel`: abort the connector's running run. */
  async cancel(id: string, projectId: string, requestedBy?: string) {
    this.assertSynxEnabled()
    const row = await this.getOwned(id, projectId)
    const command = await this.enqueueCommand(row, 'cancel', requestedBy)
    await this.finishActiveRun(id, 'stopped', null)
    await this.connectorRepository.deleteLease(id)
    return {
      ok: true,
      connectorId: id,
      commandId: command.id,
      status: command.status,
      message: 'Cancel queued for a synx worker.'
    }
  }

  async getCommand(id: string, projectId: string): Promise<ConnectorCommandRow> {
    this.assertSynxEnabled()
    const command = await this.connectorRepository.findCommandById(id)
    if (!command || command.projectId !== projectId) {
      throw new NotFoundException('Connector command not found')
    }
    return command
  }

  async listCommands(id: string, projectId: string, limit = 20): Promise<ConnectorCommandRow[]> {
    this.assertSynxEnabled()
    await this.getOwned(id, projectId)
    return this.connectorRepository.findCommands(id, limit)
  }

  /**
   * Worker-side claim: returns the oldest pending command with its connector
   * config + decrypted secrets so the worker can actually connect and run the
   * checks. A command is claimed atomically (pending → claimed) so two workers
   * never run the same test.
   */
  async claimCommand(workerId: string, tokenHeader?: string) {
    this.assertSynxEnabled()
    this.assertWorkerToken(tokenHeader)
    const [row] = await this.connectorRepository.findPendingCommand(1)
    if (!row) {
      return null
    }
    const claimed = await this.connectorRepository.claimCommand(row.id, workerId)
    if (!claimed) {
      return null
    }
    const connector = await this.connectorRepository.findById(row.connectorId)
    if (!connector) {
      return null
    }
    const secret = await this.connectorRepository.findSecret(row.connectorId)
    const storedSecrets =
      secret?.ciphertext ?
        this.connectorSecretService.decrypt<Record<string, unknown>>(secret.ciphertext)
      : {}
    const { [INTERNAL_DESTINATION_API_KEY]: _destinationApiKey, ...sourceSecrets } = storedSecrets
    return {
      commandId: claimed.id,
      type: claimed.type,
      connectorId: claimed.connectorId,
      projectId: claimed.projectId,
      payload: claimed.payload ? this.parse(claimed.payload) : undefined,
      connector: {
        name: connector.name,
        type: connector.type,
        config: this.parse(connector.config),
        secrets: sourceSecrets
      },
      workerId
    }
  }

  async completeCommand(
    commandId: string,
    result: Record<string, unknown>,
    errorMessage?: string,
    tokenHeader?: string
  ) {
    this.assertSynxEnabled()
    this.assertWorkerToken(tokenHeader)
    const command = await this.connectorRepository.findCommandById(commandId)
    if (!command) {
      throw new NotFoundException('Connector command not found')
    }
    if (command.status !== 'claimed') {
      throw new BadRequestException('Connector command is not in a claimable state')
    }
    const status = errorMessage ? 'failed' : 'completed'
    await this.connectorRepository.completeCommand(commandId, {
      status,
      result: result ? JSON.stringify(result) : undefined,
      errorMessage
    })
    const connector = await this.connectorRepository.findById(command.connectorId)
    if (connector) {
      await this.event(
        connector,
        'command',
        errorMessage ? `Command ${command.type} failed` : `Command ${command.type} completed`,
        errorMessage ? { error: errorMessage } : result
      )
    }
    return { ok: true, commandId, status }
  }

  private async enqueueCommand(row: ConnectorRow, type: ConnectorCommandType, requestedBy?: string) {
    if (!CONNECTOR_COMMAND_TYPES.includes(type)) {
      throw new BadRequestException('Unsupported connector command type')
    }
    return this.connectorRepository.createCommand({
      id: uuidv7(),
      connectorId: row.id,
      projectId: row.projectId,
      type,
      status: 'pending',
      requestedBy,
      createdAt: getCurrentISO()
    })
  }

  async events(id: string, projectId: string): Promise<ConnectorEventRow[]> {
    this.assertSynxEnabled()
    await this.getOwned(id, projectId)
    return this.connectorRepository.findEvents(id)
  }

  async claim(workerId: string, tokenHeader?: string, leaseTtlMs?: number) {
    this.assertSynxEnabled()
    this.assertWorkerToken(tokenHeader)
    const now = getCurrentISO()
    const leaseUntil = new Date(Date.now() + this.normalizeLeaseTtl(leaseTtlMs)).toISOString()
    // Atomic single-statement claim: leases the next unleased running connector
    // (no fetch-then-filter over all connectors, no double-claim between workers).
    const claimedId = await this.connectorRepository.claimNextRunnable(workerId, now, leaseUntil)
    if (!claimedId) {
      return null
    }
    const row = await this.connectorRepository.findById(claimedId)
    if (!row) {
      return null
    }
    // Record a durable run so snapshot/stream progress and history are queryable.
    const run = await this.connectorRepository.createRun({
      id: uuidv7(),
      connectorId: row.id,
      projectId: row.projectId,
      workerId,
      trigger: 'user',
      status: 'running',
      phase: 'starting',
      recordsRead: 0,
      recordsWritten: 0,
      recordsRejected: 0,
      startedAt: now,
      heartbeatAt: now
    })
    this.connectorRepository
      .update(row.id, { stats: JSON.stringify({ runId: run.id, phase: run.phase }) })
      .catch(() => undefined)
    const secret = await this.connectorRepository.findSecret(row.id)
    const storedSecrets =
      secret?.ciphertext ?
        this.connectorSecretService.decrypt<Record<string, unknown>>(secret.ciphertext)
      : {}
    const destinationApiKey = await this.getDestinationApiKey(row, storedSecrets)
    const { [INTERNAL_DESTINATION_API_KEY]: _destinationApiKey, ...sourceSecrets } = storedSecrets

    return {
      connectorId: row.id,
      projectId: row.projectId,
      name: row.name,
      type: row.type,
      generation: row.generation ?? 0,
      config: this.parse(row.config),
      transform: this.parse(row.transform),
      secrets: sourceSecrets,
      offsets: await this.connectorRepository.findOffsets(row.id),
      destination: {
        url: this.configService.get('RUSHDB_BASE_URL') ?? 'https://api.rushdb.com',
        apiKey: destinationApiKey
      },
      workerId
    }
  }

  async heartbeat(id: string, workerId: string, dto: ConnectorHeartbeatDto, tokenHeader?: string) {
    this.assertSynxEnabled()
    this.assertWorkerToken(tokenHeader)
    const row = await this.connectorRepository.findById(id)
    if (!row) {
      throw new NotFoundException('Connector not found')
    }

    const now = getCurrentISO()

    if (row.status !== 'running') {
      await this.connectorRepository.deleteLease(id)
      return { active: false, status: row.status, reason: 'connector_not_running' }
    }

    // Cheap single-statement heartbeat: renew the lease only if this worker
    // still owns it and it hasn't expired. No separate findLease round-trip,
    // and a stale worker can never extend a lease it lost.
    const renewed = await this.connectorRepository.renewLeaseIfOwned(
      id,
      workerId,
      new Date(Date.now() + this.normalizeLeaseTtl(dto.leaseTtlMs)).toISOString(),
      now
    )
    if (!renewed) {
      return { active: false, status: row.status, reason: 'lease_lost' }
    }

    if (dto.stats || typeof dto.lagMs === 'number') {
      await this.connectorRepository.update(id, {
        lagMs: dto.lagMs,
        stats: dto.stats ? JSON.stringify(dto.stats) : row.stats,
        updatedAt: now
      })
      // Surface per-run progress from the worker's heartbeat stats.
      if (dto.stats) {
        const pipelines = (dto.stats as Record<string, unknown>).pipelines as
          | Record<string, Record<string, number>>
          | undefined
        const recordsWritten = Object.values(pipelines ?? {}).reduce(
          (sum, p) => sum + Number(p?.opsApplied ?? 0),
          0
        )
        const active = await this.connectorRepository.findActiveRuns(id)
        for (const run of active) {
          await this.connectorRepository.updateRun(run.id, {
            phase: 'streaming',
            recordsWritten,
            heartbeatAt: now
          })
        }
      }
    }

    return { active: true, status: 'running' }
  }

  async release(id: string, workerId: string, tokenHeader?: string) {
    this.assertSynxEnabled()
    this.assertWorkerToken(tokenHeader)
    const lease = await this.connectorRepository.findLease(id)
    if (lease?.workerId === workerId) {
      await this.connectorRepository.deleteLease(id)
      await this.finishActiveRun(id, 'stopped', null)
    }
    return { ok: true }
  }

  async listRuns(id: string, projectId: string, limit = 20) {
    this.assertSynxEnabled()
    await this.getOwned(id, projectId)
    return this.connectorRepository.findRuns(id, limit)
  }

  async listRejections(id: string, projectId: string, limit = 50) {
    this.assertSynxEnabled()
    await this.getOwned(id, projectId)
    return this.connectorRepository.findRejections(id, limit)
  }

  async resolveRejections(id: string, projectId: string) {
    this.assertSynxEnabled()
    await this.getOwned(id, projectId)
    await this.connectorRepository.resolveRejections(id)
    return { ok: true }
  }

  /** Mark the connector's running run as finished (used on release/error). */
  private async finishActiveRun(id: string, status: 'stopped' | 'failed', errorMessage: string | null) {
    const active = await this.connectorRepository.findActiveRuns(id)
    for (const run of active) {
      await this.connectorRepository.updateRun(run.id, {
        status,
        errorMessage,
        completedAt: getCurrentISO()
      })
    }
  }

  async updateStatus(id: string, dto: ConnectorStatusDto, tokenHeader?: string, workerId?: string) {
    this.assertSynxEnabled()
    this.assertWorkerToken(tokenHeader)
    if (!CONNECTOR_STATUSES.includes(dto.status as ConnectorStatus)) {
      throw new BadRequestException('Unsupported connector status')
    }
    const row = await this.connectorRepository.findById(id)
    if (!row) {
      throw new NotFoundException('Connector not found')
    }
    const updated = await this.connectorRepository.update(id, {
      status: dto.status,
      lastError: dto.lastError,
      lagMs: dto.lagMs,
      stats: dto.stats ? JSON.stringify(dto.stats) : row.stats,
      updatedAt: getCurrentISO()
    })
    if (dto.status === 'error') {
      await this.finishActiveRun(id, 'failed', dto.lastError ?? null)
    }
    await this.event(updated, 'status', `Connector status: ${dto.status}`, dto.stats)
    return this.toPublic(updated)
  }

  async commitOffset(id: string, dto: ConnectorOffsetDto, tokenHeader?: string, workerId?: string) {
    this.assertSynxEnabled()
    this.assertWorkerToken(tokenHeader)
    const row = await this.connectorRepository.findById(id)
    if (!row) {
      throw new NotFoundException('Connector not found')
    }
    await this.assertOwnedActiveLease(id, workerId)
    await this.connectorRepository.upsertOffset({
      connectorId: id,
      partition: dto.partition,
      position: JSON.stringify(dto.position),
      updatedAt: getCurrentISO()
    })
    return { ok: true }
  }

  private async getOwned(id: string, projectId: string): Promise<ConnectorRow> {
    const row = await this.connectorRepository.findById(id)
    if (!row || row.projectId !== projectId) {
      throw new NotFoundException('Connector not found')
    }
    return row
  }

  private async assertValidType(type: string): Promise<void> {
    // Database connectors are the legacy hardcoded set; any other type is
    // valid only if a worker-registered spec declares it (spec connectors are
    // never hardcoded here).
    if (CONNECTOR_TYPES.includes(type as ConnectorType)) {
      return
    }
    if (await this.connectorRegistry.isRegistered(type)) {
      return
    }
    throw new BadRequestException('Unsupported connector type')
  }

  private validateConfig(type: ConnectorType | string, config: Record<string, unknown>) {
    if (type === 'postgres' || type === 'mysql') {
      if (!config.host && !config.connString && !config.url) {
        throw new BadRequestException(
          type === 'postgres' ?
            'PostgreSQL connector needs host or connString'
          : 'MySQL connector needs host or url'
        )
      }
      return
    }
    if (type === 'mongodb') {
      if (!config.uri && !config.host) {
        throw new BadRequestException('MongoDB connector needs uri or host')
      }
      return
    }
    // Schema-driven spec connectors carry a baseUrl (provider default applies
    // when absent) and an access token in secrets.
    if (typeof type === 'string') {
      return
    }
    throw new BadRequestException('Unsupported connector type')
  }

  private normalizeTransform(input?: Record<string, unknown>): ConnectorTransform {
    return {
      naming: 'preserve',
      singularize: true,
      mergeStrategy: 'append',
      ...(input ?? {})
    } as ConnectorTransform
  }

  private async storeSecrets(connectorId: string, secrets: Record<string, unknown>) {
    const existing = await this.connectorRepository.findSecret(connectorId)
    const existingSecrets =
      existing?.ciphertext ?
        this.connectorSecretService.decrypt<Record<string, unknown>>(existing.ciphertext)
      : {}
    const internalDestinationApiKey = existingSecrets[INTERNAL_DESTINATION_API_KEY]

    await this.connectorRepository.upsertSecret({
      connectorId,
      provider: 'local',
      ciphertext: this.connectorSecretService.encrypt({
        ...secrets,
        ...(internalDestinationApiKey ? { [INTERNAL_DESTINATION_API_KEY]: internalDestinationApiKey } : {})
      }),
      secretRef: null,
      createdAt: getCurrentISO(),
      updatedAt: getCurrentISO()
    })
  }

  private async getDestinationApiKey(row: ConnectorRow, secrets: Record<string, unknown>): Promise<string> {
    const stored = secrets[INTERNAL_DESTINATION_API_KEY]
    if (typeof stored === 'string' && stored) {
      return stored
    }

    const bootstrapKey = this.configService.get<string>('RUSHDB_SYNX_DESTINATION_API_KEY')
    if (bootstrapKey) {
      return bootstrapKey
    }

    const token = await this.tokenService.createToken(
      {
        name: `synx:${row.id}`,
        description: 'Project-scoped internal token for managed synx connector writes',
        expiration: '*',
        level: 'write'
      },
      row.projectId
    )
    const apiKey = token.toJson().value
    await this.storeSecrets(row.id, { ...secrets, [INTERNAL_DESTINATION_API_KEY]: apiKey })
    return apiKey
  }

  private normalizeLeaseTtl(value?: number) {
    if (!Number.isFinite(value)) {
      return 60_000
    }
    return Math.min(Math.max(value, 15_000), 300_000)
  }

  private async assertOwnedActiveLease(connectorId: string, workerId?: string) {
    const lease = await this.connectorRepository.findLease(connectorId)
    const now = getCurrentISO()
    if (!lease || lease.leaseUntil <= now || lease.workerId !== workerId) {
      if (lease?.leaseUntil <= now) {
        await this.connectorRepository.deleteLease(connectorId)
      }
      throw new ForbiddenException('Connector lease is not owned by this worker')
    }
  }

  private async event(
    row: ConnectorRow,
    type: string,
    message: string,
    metadata?: Record<string, unknown>
  ): Promise<void> {
    if (!row) {
      return
    }
    await this.connectorRepository.addEvent({
      id: uuidv7(),
      connectorId: row.id,
      projectId: row.projectId,
      level: type === 'status' && row.status === 'error' ? 'error' : 'info',
      type,
      message,
      metadata: metadata ? JSON.stringify(metadata) : null,
      createdAt: getCurrentISO()
    })
  }

  private toPublic(row: ConnectorRow): PublicConnector {
    return {
      ...row,
      config: this.parse(row.config),
      transform: this.parse(row.transform),
      stats: row.stats ? this.parse(row.stats) : undefined,
      health: deriveConnectorHealth(row),
      secrets: { value: '••••' }
    }
  }

  private parse(value: string): Record<string, unknown> {
    try {
      return JSON.parse(value)
    } catch {
      return {}
    }
  }

  private assertWorkerToken(tokenHeader?: string) {
    const token = this.configService.get<string>('RUSHDB_SYNX_CONTROL_TOKEN')
    if (!token) {
      throw new ForbiddenException('Synx control token is not configured')
    }
    if (tokenHeader !== token) {
      throw new ForbiddenException('Invalid synx control token')
    }
  }

  private assertSynxEnabled() {
    if (!this.configService.get('RUSHDB_SYNX_CONTROL_TOKEN')) {
      throw new ServiceUnavailableException('Continuous sync is not configured')
    }
  }
}
