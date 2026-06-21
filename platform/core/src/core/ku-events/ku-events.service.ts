import { HttpService } from '@nestjs/axios'
import { Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'

import { toBoolean } from '@/common/utils/toBolean'
import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { KuOperationEvent } from '@/core/ku-events/ku-events.interface'

@Injectable()
export class KuEventsService {
  private readonly logger = new Logger(KuEventsService.name)

  constructor(
    private readonly httpService: HttpService,
    private readonly configService: ConfigService
  ) {}

  /**
   * Emits a KU operation event to the external billing service.
   *
   * This is fire-and-forget: errors are swallowed silently to never
   * block or slow down the main application flow.
   *
   * No-ops when:
   * - RUSHDB_SELF_HOSTED=true  (unlimited self-hosted mode)
   * - BILLING_SERVICE_URL is not configured  (no billing service)
   *
   * @param workspaceId - The workspace ID (billing tenant)
   * @param projectId - The project ID (attribution/analytics)
   * @param operation - The KU operation type
   * @param metadata - Additional context for the operation
   */
  emit(
    workspaceId: string,
    projectId: string,
    operation: KuOperation,
    metadata?: Record<string, unknown>
  ): void {
    if (toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
      return
    }

    const billingUrl = this.configService.get<string>('BILLING_SERVICE_URL')
    if (!billingUrl) {
      return
    }

    const event: KuOperationEvent = {
      workspaceId,
      projectId,
      operation,
      metadata,
      timestamp: Date.now()
    }

    const secret = this.configService.get<string>('RUSHDB_BILLING_SECRET')
    const headers: Record<string, string> = { 'Content-Type': 'application/json' }
    if (secret) {
      headers['x-rushdb-billing-secret'] = secret
    }

    // Fire-and-forget: dispatch outside the current request call stack and never throw.
    setImmediate(() => {
      this.httpService
        .post(`${billingUrl}/api/ku-events`, event, { headers })
        .toPromise()
        .catch((err: Error) => {
          this.logger.warn(`KU event delivery failed [${operation}]: ${err?.message}`)
        })
    })
  }

  /**
   * Convenience method for bulk emissions (e.g., after a batch ingestion).
   * Coalesces count into metadata to avoid N HTTP calls for N records.
   */
  emitBulk(
    workspaceId: string,
    projectId: string,
    operation: KuOperation,
    count: number,
    metadata?: Record<string, unknown>
  ): void {
    this.emit(workspaceId, projectId, operation, { count, ...metadata })
  }
}
