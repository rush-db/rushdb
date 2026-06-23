import { Injectable, Logger } from '@nestjs/common'

import { getCurrentISO } from '@/common/utils/getCurrentISO'

export type TAuthEvent =
  | 'sso.login.success'
  | 'sso.login.failed'
  | 'sso.config.updated'
  | 'sso.config.deleted'

/**
 * Lightweight, append-only audit trail for authentication-sensitive events.
 *
 * MVP implementation emits structured log lines so they can be shipped to any
 * log aggregator. The shape is intentionally stable so it can later be backed
 * by a dedicated `auth_events` table without changing call sites.
 */
@Injectable()
export class AuditService {
  private readonly logger = new Logger('AuthAudit')

  record(
    event: TAuthEvent,
    context: {
      workspaceId?: string
      userId?: string
      email?: string
      provider?: string
      reason?: string
    } = {}
  ): void {
    this.logger.log(JSON.stringify({ event, at: getCurrentISO(), ...context }))
  }
}
