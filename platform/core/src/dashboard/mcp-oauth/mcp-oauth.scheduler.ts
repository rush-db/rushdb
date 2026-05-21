import { Injectable, Logger } from '@nestjs/common'
import { Cron, CronExpression } from '@nestjs/schedule'

import { OAuthRepository } from '@/dashboard/mcp-oauth/model/oauth.repository'
import { TokenRepository } from '@/dashboard/token/model/token.repository'

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
      const [tokens, authReqs, codes, refreshTokens] = await Promise.all([
        this.tokenRepository.deleteExpired(),
        this.oauthRepository.deleteExpiredAuthRequests(),
        this.oauthRepository.deleteExpiredCodes(),
        this.oauthRepository.deleteExpiredRefreshTokens()
      ])

      if (tokens + authReqs + codes + refreshTokens > 0) {
        this.logger.log(
          `[OAuth cleanup] deleted ${tokens} expired tokens, ${authReqs} auth requests, ${codes} auth codes, ${refreshTokens} refresh tokens`
        )
      }
    } catch (err) {
      this.logger.error('[OAuth cleanup] scheduled cleanup failed', err)
    }
  }
}
