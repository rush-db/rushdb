import { Injectable, NestMiddleware, Logger } from '@nestjs/common'
import { Response, NextFunction } from 'express'

import { PlatformRequest } from '@/common/types/request'
import { isDevMode } from '@/common/utils/isDevMode'
import { extractMixedPropertiesFromToken } from '@/common/utils/tokenUtils'
import { AuthService } from '@/dashboard/auth/auth.service'
import { TokenService } from '@/dashboard/token/token.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
export class AuthMiddleware implements NestMiddleware {
  constructor(
    private readonly authService: AuthService,
    private readonly tokenService: TokenService,
    private readonly neogmaService: NeogmaService
  ) {}

  async use(request: PlatformRequest, response: Response, next: NextFunction) {
    if (request.method === 'OPTIONS') {
      return next()
    }

    // Fastify-related workaround.
    if (request.user || request.projectId) {
      return next()
    }

    try {
      const authHeader = request.headers['authorization']
      const bearerToken = authHeader?.split(' ')[1]
      const isJwt = bearerToken?.split('.').length === 3

      // Flow for SDK auth (token-based)
      if (request.headers['token'] || !isJwt) {
        let session = this.neogmaService.createSession('auth-middleware-sdk')
        let transaction = session.beginTransaction()
        try {
          const tokenHeader = (request.headers['token'] || bearerToken) as string
          if (tokenHeader) {
            const tokenId = this.tokenService.decrypt(tokenHeader)
            const prefixedToken = extractMixedPropertiesFromToken(tokenHeader)

            const { hasAccess, projectId, project, workspaceId, workspace } =
              await this.tokenService.validateToken({
                tokenId,
                transaction,
                prefixData: prefixedToken
              })

            if (hasAccess) {
              // Custom properties will be accessible at request.raw.*
              // Always attach to fastify raw object so interceptors / filters can see them
              const raw: any = (request as any).raw ?? request
              raw.project = project
              raw.projectId = projectId
              raw.workspace = workspace
              raw.workspaceId = workspaceId
              raw.session = session
              raw.transaction = transaction

              return next()
            }
          }
        } catch (e) {
          isDevMode(() => Logger.error('SDK auth failed in middleware', e))
          // Rollback & close on failure to avoid leaks
          try {
            if (transaction?.isOpen?.()) await transaction.rollback()
          } catch {}
          try {
            await transaction?.close?.()
          } catch {}
          try {
            await session?.close?.()
          } catch {}
        } finally {
          // If not attached (no projectId assigned) ensure session closed
          const raw: any = (request as any).raw ?? request
          if (raw.session !== session) {
            try {
              await transaction?.close?.()
            } catch {}
            try {
              await session?.close?.()
            } catch {}
          }
        }
      }

      // Flow for JWT auth (dashboard)
      if (isJwt && authHeader) {
        let session = this.neogmaService.createSession('auth-middleware-jwt')
        let transaction = session.beginTransaction()
        try {
          const token = authHeader.split(' ')[1]
          const user = this.authService.verifyJwt(token)

          if (user) {
            // Custom properties will be accessible at request.raw.*
            const raw: any = (request as any).raw ?? request
            raw.user = user

            const { hasAccess, projectId, project, workspaceId, workspace } =
              await this.tokenService.verifyIntegrity({
                user,
                projectId: request.headers['x-project-id'] as string,
                workspaceId: request.headers['x-workspace-id'] as string,
                transaction
              })

            // Custom properties will be accessible at request.raw.*
            raw.project = project
            raw.projectId = projectId
            raw.workspace = workspace
            raw.workspaceId = workspaceId
            raw.session = session
            raw.transaction = transaction

            return next()
          }
        } catch (e) {
          isDevMode(() => Logger.error('JWT auth failed in middleware', e))
          try {
            if (transaction?.isOpen?.()) await transaction.rollback()
          } catch {}
          try {
            await transaction?.close?.()
          } catch {}
          try {
            await session?.close?.()
          } catch {}
        } finally {
          const raw: any = (request as any).raw ?? request
          if (raw.session !== session) {
            try {
              await transaction?.close?.()
            } catch {}
            try {
              await session?.close?.()
            } catch {}
          }
        }
      }
      // Continue without authentication for public endpoints
      next()
    } catch (e) {
      isDevMode(() => Logger.error('Auth middleware error', e))
      next()
    }
  }
}
