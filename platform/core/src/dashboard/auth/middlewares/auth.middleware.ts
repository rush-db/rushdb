import { Injectable, NestMiddleware, Logger } from '@nestjs/common'
import { Response, NextFunction } from 'express'
import { Session } from 'neo4j-driver'

import { PlatformRequest } from '@/common/types/request'
import { isDevMode } from '@/common/utils/isDevMode'
import { AuthService } from '@/dashboard/auth/auth.service'
import { TokenService } from '@/dashboard/token/token.service'
import { NeogmaService } from '@/database/neogma/neogma.service'
import { DEFAULT_TRANSACTION_TIMEOUT_MS } from '@/database/transaction.constants'

@Injectable()
export class AuthMiddleware implements NestMiddleware {
  constructor(
    private readonly authService: AuthService,
    private readonly tokenService: TokenService,
    private readonly neogmaService: NeogmaService
  ) {}

  async use(request: PlatformRequest, response: Response, next: NextFunction) {
    if (request.method === 'OPTIONS' || request.method === 'HEAD') {
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
      if (bearerToken && !isJwt) {
        let session: Session | undefined
        try {
          const tokenId = this.tokenService.decrypt(bearerToken)

          const { hasAccess, projectId, project, workspaceId, workspace, accessLevel, canWrite } =
            await this.tokenService.validateToken({
              tokenId
            })

          if (hasAccess) {
            // Read-only tokens get a READ-mode session so Neo4j itself rejects
            // any write, regardless of route-level authorization.
            session = this.neogmaService.createSession('auth-middleware-sdk', canWrite ? 'WRITE' : 'READ')
            const transaction = session.beginTransaction({ timeout: DEFAULT_TRANSACTION_TIMEOUT_MS })

            // Custom properties will be accessible at request.raw.*
            // Always attach to fastify raw object so interceptors / filters can see them
            const raw: any = (request as any).raw ?? request
            raw.project = project
            raw.projectId = projectId
            raw.workspace = workspace
            raw.workspaceId = workspaceId
            raw.session = session
            raw.transaction = transaction
            raw.tokenAccessLevel = accessLevel
            raw.tokenCanWrite = canWrite

            return next()
          }
        } catch (e) {
          isDevMode(() => Logger.error('SDK auth failed in middleware', e))
          if (session) {
            try {
              await this.neogmaService.closeSession(session)
            } catch {
              /* empty */
            }
          }
        }
      }

      // Flow for JWT auth (dashboard)
      if (isJwt && authHeader) {
        const session = this.neogmaService.createSession('auth-middleware-jwt')
        const transaction = session.beginTransaction({ timeout: DEFAULT_TRANSACTION_TIMEOUT_MS })
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
                workspaceId: request.headers['x-workspace-id'] as string
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
            if (transaction?.isOpen?.()) {
              await transaction.rollback()
            }
          } catch {
            /* empty */
          }
          try {
            await transaction?.close?.()
          } catch {
            /* empty */
          }
          try {
            await session?.close?.()
          } catch {
            /* empty */
          }
        } finally {
          const raw: any = (request as any).raw ?? request
          if (raw.session !== session) {
            try {
              await transaction?.close?.()
            } catch {
              /* empty */
            }
            try {
              await this.neogmaService.closeSession(session)
            } catch {
              /* empty */
            }
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
