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

    const session = this.neogmaService.createSession('auth-middleware')
    const transaction = session.beginTransaction()

    const shouldPerformCleanUp = false

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

            const { hasAccess, projectId, project, workspaceId, workspace } =
              await this.tokenService.validateToken({
                tokenId,
                transaction,
                prefixData: prefixedToken
              })

            if (hasAccess) {
              // Custom properties will be accessible at request.raw.*
              request.project = project
              request.projectId = projectId
              request.workspace = workspace
              request.workspaceId = workspaceId

              // if (project?.customDb) {
              //   shouldPerformCleanUp = true
              // } else {
              request.session = session
              request.transaction = transaction
              // }

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

            const { hasAccess, projectId, project, workspaceId, workspace } =
              await this.tokenService.verifyIntegrity({
                user,
                projectId: request.headers['x-project-id'] as string,
                workspaceId: request.headers['x-workspace-id'] as string,
                transaction
              })

            // Custom properties will be accessible at request.raw.*
            request.project = project
            request.projectId = projectId
            request.workspace = workspace
            request.workspaceId = workspaceId

            // Check if a project relies on external db
            // if (project?.customDb || project?.managedDbPassword) {
            //   shouldPerformCleanUp = true
            // } else {
            request.session = session
            request.transaction = transaction
            // }

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
      isDevMode(() => Logger.log('Should perform session and transaction cleanups', shouldPerformCleanUp))

      // if (shouldPerformCleanUp) {
      //   await cleanUp()
      // }
    }
  }
}
