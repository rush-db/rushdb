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
import { ConnectorSecretService } from '@/dashboard/connector/connector-secret.service'
import {
  CONNECTOR_STATUSES,
  CONNECTOR_TYPES,
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
import { TokenService } from '@/dashboard/token/token.service'

import type { ConnectorEventRow, ConnectorRow } from '@/database/sql/schema/types'

const INTERNAL_DESTINATION_API_KEY = '__rushdbDestinationApiKey'

type PublicConnector = Omit<ConnectorRow, 'config' | 'transform' | 'stats'> & {
  config: Record<string, unknown>
  transform: ConnectorTransform
  stats?: Record<string, unknown>
  secrets: Record<string, '••••'>
}

@Injectable()
export class ConnectorService {
  constructor(
    private readonly connectorRepository: ConnectorRepository,
    private readonly connectorSecretService: ConnectorSecretService,
    private readonly projectService: ProjectService,
    private readonly tokenService: TokenService,
    private readonly configService: ConfigService
  ) {}

  async create(dto: CreateConnectorDto, projectId: string, createdBy?: string): Promise<PublicConnector> {
    this.assertSynxEnabled()
    await this.projectService.getProject(projectId)
    this.validateType(dto.type)
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

  async delete(id: string, projectId: string): Promise<boolean> {
    this.assertSynxEnabled()
    const row = await this.getOwned(id, projectId)
    await this.event(row, 'deleted', 'Connector deleted')
    return this.connectorRepository.delete(id)
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
    await this.event(row, 'resnapshot_requested', 'Connector resnapshot requested')
    return this.toPublic(row)
  }

  async test(id: string, projectId: string) {
    this.assertSynxEnabled()
    const row = await this.getOwned(id, projectId)
    this.validateConfig(row.type as ConnectorType, this.parse(row.config))
    const secret = await this.connectorRepository.findSecret(id)
    await this.event(row, 'test', 'Connector configuration accepted')
    return {
      ok: true,
      connectorId: id,
      checkedAt: getCurrentISO(),
      hasSecrets: Boolean(secret?.ciphertext || secret?.secretRef),
      message: 'Configuration is valid. Live network credential checks run in synx worker managed mode.'
    }
  }

  async events(id: string, projectId: string): Promise<ConnectorEventRow[]> {
    this.assertSynxEnabled()
    await this.getOwned(id, projectId)
    return this.connectorRepository.findEvents(id)
  }

  async claim(workerId: string, tokenHeader?: string, leaseTtlMs?: number) {
    this.assertSynxEnabled()
    this.assertWorkerToken(tokenHeader)
    const [row] = await this.connectorRepository.findRunnableWithoutLease(1)
    if (!row) {
      return null
    }
    const now = getCurrentISO()
    const leaseUntil = new Date(Date.now() + this.normalizeLeaseTtl(leaseTtlMs)).toISOString()
    await this.connectorRepository.upsertLease({
      connectorId: row.id,
      workerId,
      leaseUntil,
      heartbeatAt: now,
      createdAt: now,
      updatedAt: now
    })
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

    const lease = await this.connectorRepository.findLease(id)
    const now = getCurrentISO()

    if (!lease || lease.workerId !== workerId || lease.leaseUntil <= now) {
      if (lease?.leaseUntil <= now) {
        await this.connectorRepository.deleteLease(id)
      }
      return { active: false, status: row.status, reason: 'lease_lost' }
    }

    if (row.status !== 'running') {
      await this.connectorRepository.deleteLease(id)
      return { active: false, status: row.status, reason: 'connector_not_running' }
    }

    await this.connectorRepository.upsertLease({
      connectorId: id,
      workerId,
      leaseUntil: new Date(Date.now() + this.normalizeLeaseTtl(dto.leaseTtlMs)).toISOString(),
      heartbeatAt: now,
      createdAt: lease.createdAt,
      updatedAt: now
    })

    if (dto.stats || typeof dto.lagMs === 'number') {
      await this.connectorRepository.update(id, {
        lagMs: dto.lagMs,
        stats: dto.stats ? JSON.stringify(dto.stats) : row.stats,
        updatedAt: now
      })
    }

    return { active: true, status: 'running' }
  }

  async release(id: string, workerId: string, tokenHeader?: string) {
    this.assertSynxEnabled()
    this.assertWorkerToken(tokenHeader)
    const lease = await this.connectorRepository.findLease(id)
    if (lease?.workerId === workerId) {
      await this.connectorRepository.deleteLease(id)
    }
    return { ok: true }
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

  private validateType(type: string): asserts type is ConnectorType {
    if (!CONNECTOR_TYPES.includes(type as ConnectorType)) {
      throw new BadRequestException('Unsupported connector type')
    }
  }

  private validateConfig(type: ConnectorType | string, config: Record<string, unknown>) {
    if (type === 'postgres') {
      if (!config.host && !config.connString) {
        throw new BadRequestException('PostgreSQL connector needs host or connString')
      }
      if (!config.database && !config.connString) {
        throw new BadRequestException('PostgreSQL connector needs database')
      }
      return
    }
    if (type === 'mongodb') {
      if (!config.uri && !config.host) {
        throw new BadRequestException('MongoDB connector needs uri or host')
      }
      if (!config.database) {
        throw new BadRequestException('MongoDB connector needs database')
      }
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
