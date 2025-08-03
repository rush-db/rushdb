import { Injectable, NestMiddleware, Logger } from '@nestjs/common'
import { Response, NextFunction } from 'express'

import { PlatformRequest } from '@/common/types/request'
import { isDevMode } from '@/common/utils/isDevMode'
import { extractMixedPropertiesFromToken } from '@/common/utils/tokenUtils'
import { AuthService } from '@/dashboard/auth/auth.service'
import { TokenService } from '@/dashboard/token/token.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

type ExtendedPlatformRequest = PlatformRequest & {
  workspaceId?: string
}

@Injectable()
export class AuthMiddleware implements NestMiddleware {
  constructor(
    private readonly authService: AuthService,
    private readonly tokenService: TokenService,
    private readonly neogmaService: NeogmaService
  ) {}

  async use(request: ExtendedPlatformRequest, response: Response, next: NextFunction) {
    const session = this.neogmaService.createSession('auth-middleware')
    const transaction = session.beginTransaction()

    const cleanUp = async () => {
      await transaction.close()
      await this.neogmaService.closeSession(session, 'auth-middleware')
    }

    try {
      const authHeader = request.headers['authorization']
      const bearerToken = authHeader?.split(' ')[1]
      const isJwt = bearerToken?.split('.').length === 3

      // Flow for SDK auth (token-based)
      if (request.headers['token'] || !isJwt) {
        try {
          const tokenHeader = (request.headers['token'] || bearerToken) as string
          if (tokenHeader) {
            const tokenId = this.tokenService.decrypt(tokenHeader)
            const prefixedToken = extractMixedPropertiesFromToken(tokenHeader)

            const { hasAccess, projectId, workspaceId } = await this.tokenService.validateToken({
              tokenId,
              transaction,
              prefixData: prefixedToken
            })

            if (hasAccess) {
              // Custom properties will be accessible at request.raw.*
              request.projectId = projectId
              request.workspaceId = workspaceId
              return next()
            }
          }
        } catch (e) {
          isDevMode(() => Logger.error('SDK auth failed in middleware', e))
        }
      }

      // Flow for JWT auth (dashboard)
      if (isJwt && authHeader) {
        try {
          const token = authHeader.split(' ')[1]
          const user = this.authService.verifyJwt(token)

          if (user) {
            // Custom properties will be accessible at request.raw.*
            request.user = user

            // Try to extract project ID from headers for dashboard requests
            const projectId = request.headers['x-project-id'] as string
            if (projectId) {
              request.projectId = projectId
            }

            return next()
          }
        } catch (e) {
          isDevMode(() => Logger.error('JWT auth failed in middleware', e))
        }
      }

      // Continue without authentication for public endpoints
      next()
    } catch (e) {
      isDevMode(() => Logger.error('Auth middleware error', e))
      next()
    } finally {
      await cleanUp()
    }
  }
}
