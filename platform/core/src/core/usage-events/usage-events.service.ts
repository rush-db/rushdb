import { HttpService } from '@nestjs/axios'
import { Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { firstValueFrom } from 'rxjs'

import { toBoolean } from '@/common/utils/toBolean'
import { UsageEventOutboxService } from '@/core/usage-events/usage-event-outbox.service'
import {
  UsageOperation,
  DeploymentKind,
  MEASUREMENT_VERSION,
  USAGE_EVENT_SOURCES,
  UsageEventOutcome,
  UsageEventSource
} from '@/core/usage-events/usage-events.constants'
import { UsageEventV3 } from '@/core/usage-events/usage-events.interface'
import { buildIdempotencyKey } from '@/core/usage-events/write-measurements'

import { randomUUID } from 'node:crypto'

interface EmitUsageEventInput {
  workspaceId: string
  projectId?: string
  operationClass: UsageOperation
  routeOrTool: string
  source?: UsageEventSource
  outcome?: UsageEventOutcome
  /** Request correlation id; one external request always maps to one event. */
  requestId?: string
  deployment?: DeploymentKind
  costDimensions?: UsageEventV3['costDimensions']
}

/**
 * Records usage telemetry events and persists them durably in the outbox
 * until the configured delivery endpoint acknowledges them.
 *
 * No-ops when RUSHDB_SELF_HOSTED=true or BILLING_SERVICE_URL is not configured.
 */
@Injectable()
export class UsageEventsService {
  private readonly logger = new Logger(UsageEventsService.name)

  constructor(
    private readonly httpService: HttpService,
    private readonly configService: ConfigService,
    private readonly outbox: UsageEventOutboxService
  ) {}

  isEnabled(): boolean {
    if (toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
      return false
    }
    return Boolean(this.configService.get<string>('BILLING_SERVICE_URL'))
  }

  /**
   * Builds a v3 usage event and enqueues it in the durable outbox.
   * Never throws: telemetry failures never fail the operation that
   * produced them.
   */
  async emit(input: EmitUsageEventInput): Promise<void> {
    try {
      if (!this.isEnabled()) {
        return
      }

      const requestId = input.requestId ?? randomUUID()
      const event: UsageEventV3 = {
        schemaVersion: 3,
        eventId: randomUUID(),
        idempotencyKey: buildIdempotencyKey(
          input.workspaceId,
          input.projectId ?? '',
          requestId,
          input.operationClass
        ),
        workspaceId: input.workspaceId,
        projectId: input.projectId,
        occurredAt: new Date().toISOString(),
        operationClass: input.operationClass,
        routeOrTool: input.routeOrTool,
        source: this.normalizeSource(input.source),
        outcome: input.outcome ?? 'succeeded',
        costDimensions: this.sanitizeCostDimensions(input.costDimensions ?? {}),
        classificationVersion: MEASUREMENT_VERSION,
        deployment: input.deployment ?? 'managed',
        metadata: undefined
      }

      await this.outbox.enqueue(event)
    } catch (error) {
      this.logger.warn(`Failed to enqueue usage event [${input.operationClass}]: ${error?.message}`)
    }
  }

  /**
   * Delivers one batch of due events to the configured delivery endpoint.
   * Returns the number of acknowledged events. Delivery failures only
   * reschedule — they never propagate.
   */
  async flushDueEvents(): Promise<number> {
    if (!this.isEnabled()) {
      return 0
    }

    const rows = await this.outbox.claimDueBatch()
    if (rows.length === 0) {
      return 0
    }

    const events = rows.map((row) => JSON.parse(row.payload) as UsageEventV3)

    try {
      const deliveryUrl = this.configService.get<string>('BILLING_SERVICE_URL')
      const secret = this.configService.get<string>('RUSHDB_BILLING_SECRET')
      const headers: Record<string, string> = { 'Content-Type': 'application/json' }
      if (secret) {
        headers['x-rushdb-billing-secret'] = secret
      }

      await firstValueFrom(
        this.httpService.post(`${deliveryUrl}/api/usage-events/v3/batch`, { events }, { headers })
      )

      await this.outbox.markDelivered(rows.map((row) => row.id))
      return rows.length
    } catch (error) {
      await this.outbox.markFailed(
        rows.map((row) => ({ row, error: error?.message ?? 'unknown delivery error' }))
      )
      return 0
    }
  }

  private normalizeSource(source?: UsageEventSource): UsageEventSource {
    return source && USAGE_EVENT_SOURCES.includes(source) ? source : 'rest'
  }

  /**
   * Drops undefined cost dimensions so events stay compact and never carry
   * accidental payload data. Only known dimension keys survive.
   */ private sanitizeCostDimensions(
    dimensions: UsageEventV3['costDimensions']
  ): UsageEventV3['costDimensions'] {
    const sanitized: UsageEventV3['costDimensions'] = {}
    for (const [key, value] of Object.entries(dimensions)) {
      if (value !== undefined) {
        sanitized[key] = value
      }
    }
    return sanitized
  }
}
