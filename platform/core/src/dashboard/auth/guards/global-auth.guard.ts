import {
  applyDecorators,
  CanActivate,
  ExecutionContext,
  Injectable,
  SetMetadata,
  UseGuards
} from '@nestjs/common'
import { Reflector } from '@nestjs/core'

import { AuthService } from '@/dashboard/auth/auth.service'
import { ProjectService } from '@/dashboard/project/project.service'
import { ThrottleByTokenGuard } from '@/dashboard/throttle/guards/throttle-by-token.guard'
import { TokenService } from '@/dashboard/token/token.service'
import { UserService } from '@/dashboard/user/user.service'
import { NeogmaService } from '@/database/neogma/neogma.service'
import { USER_ROLE_EDITOR } from '@/dashboard/user/interfaces/user.constants'
import { TUserRoles } from '@/dashboard/user/model/user.interface'

type TDashboardTargetType = 'project' | 'workspace'

@Injectable()
class GlobalAuthGuard implements CanActivate {
  constructor(
    private reflector: Reflector,
    readonly userService: UserService,
    readonly authService: AuthService,
    readonly tokenService: TokenService,
    readonly projectService: ProjectService,
    readonly neogmaService: NeogmaService
  ) {}

  async canActivate(context: ExecutionContext): Promise<boolean> {
    const request = context.switchToHttp().getRequest()
    const dashboardTargetType = this.reflector.get<TDashboardTargetType | null | undefined>(
      'dashboardTargetType',
      context.getHandler()
    )

    const minimalRole =
      this.reflector.get<TUserRoles>('minimalRole', context.getHandler()) ?? USER_ROLE_EDITOR

    const session = this.neogmaService.createSession()
    const transaction = session.beginTransaction()

    const cleanUp = async () => {
      await transaction.close()
      await this.neogmaService.closeSession(session, 'global-auth-guard')
    }

    if (request.headers['token']) {
      try {
        const authHeader = request.headers['token']
        const tokenId = this.tokenService.decrypt(authHeader)

        const { hasAccess, projectId, workspaceId } = await this.tokenService.validateToken({
          tokenId,
          transaction
        })

        if (hasAccess) {
          request.projectId = projectId
          request.workspaceId = workspaceId
          return true
        }
      } catch (e) {
        return false
      } finally {
        await cleanUp()
      }
    }

    const checkDashboardAccess = async () => {
      try {
        const request = context.switchToHttp().getRequest()
        const authHeader = request.headers['authorization']
        const token = authHeader.split(' ')[1]
        const user = this.authService.verifyJwt(token)
        if (!user) {
          throw new Error('no user')
        }
        request.user = user

        const userId = user?.id

        // detect root requests and exit from the guard
        if (dashboardTargetType === null && userId) {
          return true
        }

        const targetId =
          dashboardTargetType === 'workspace' ?
            request.headers['x-workspace-id']
          : request.headers['x-project-id']

        if (!targetId || !userId) {
          return false
        }

        const hasAccess = await this.userService.hasMinimalAccessLevel({
          userId,
          targetId,
          targetType: dashboardTargetType,
          accessLevel: minimalRole,
          transaction
        })

        if (hasAccess && dashboardTargetType === 'project') {
          request.projectId = targetId
        }

        return hasAccess
      } catch (e) {
        return false
      } finally {
        await cleanUp()
      }
    }

    return await checkDashboardAccess()
  }
}

export const AuthGuard = (
  dashboardTargetType: TDashboardTargetType | null = 'workspace',
  minimalRole: TUserRoles = USER_ROLE_EDITOR
) =>
  applyDecorators(
    UseGuards(GlobalAuthGuard, ThrottleByTokenGuard),
    SetMetadata('dashboardTargetType', dashboardTargetType),
    SetMetadata('minimalRole', minimalRole)
  )
