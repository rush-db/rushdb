import { Injectable, Logger } from '@nestjs/common'
import { Cron, CronExpression } from '@nestjs/schedule'

import { TokenRepository } from '@/dashboard/token/model/token.repository'
import { OAuthRepository } from '@/dashboard/mcp-oauth/model/oauth.repository'

@Injectable()
export class McpOauthScheduler {
  private readonly logger = new Logger(McpOauthScheduler.name)

  constructor(
    private readonly tokenRepository: TokenRepository,
    private readonly oauthRepository: OAuthRepository
  ) {}

  @Cron(CronExpression.EVERY_HOUR)
  async cleanupExpiredOAuthNodes(): Promise<void> {
    try {
      const [tokens, authReqs, codes] = await Promise.all([
        this.tokenRepository.deleteExpired(),
        this.oauthRepository.deleteExpiredAuthRequests(),
        this.oauthRepository.deleteExpiredCodes()
      ])

      if (tokens + authReqs + codes > 0) {
        this.logger.log(
          `[OAuth cleanup] deleted ${tokens} expired tokens, ${authReqs} auth requests, ${codes} auth codes`
        )
      }
    } catch (err) {
      this.logger.error('[OAuth cleanup] scheduled cleanup failed', err)
    }
  }
}
