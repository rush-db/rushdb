import {
  applyDecorators,
  CanActivate,
  ExecutionContext,
  Injectable,
  SetMetadata,
  UseGuards
} from '@nestjs/common'
import { Reflector } from '@nestjs/core'

import { ThrottleByTokenGuard } from '@/dashboard/throttle/guards/throttle-by-token.guard'
import { USER_ROLE_EDITOR } from '@/dashboard/user/interfaces/user.constants'
import { TUserRoles } from '@/dashboard/user/model/user.interface'
import { UserService } from '@/dashboard/user/user.service'

type TDashboardTargetType = 'project' | 'workspace'

@Injectable()
class GlobalAuthGuard implements CanActivate {
  constructor(
    private reflector: Reflector,
    readonly userService: UserService
  ) {}

  async canActivate(context: ExecutionContext): Promise<boolean> {
    const request = context.switchToHttp().getRequest()
    const dashboardTargetType = this.reflector.get<TDashboardTargetType | null | undefined>(
      'dashboardTargetType',
      context.getHandler()
    )

    const minimalRole =
      this.reflector.get<TUserRoles>('minimalRole', context.getHandler()) ?? USER_ROLE_EDITOR

    // false by default (will not allow to bypass)
    const optionalGuard = this.reflector.get<boolean>('optionalGuard', context.getHandler()) ?? false

    const authHeader = request.headers['authorization']
    const bearerToken = authHeader?.split(' ')[1]
    const isJwt = bearerToken?.split('.').length === 3

    if ((request.headers['token'] || !isJwt) && request.raw.projectId && request.raw.workspaceId) {
      request.projectId = request.raw.projectId
      request.workspaceId = request.raw.workspaceId
      return true
    }

    const checkDashboardAccess = async () => {
      try {
        const request = context.switchToHttp().getRequest()
        const user = request.raw.user
        if (!user) {
          throw new Error('no user')
        }
        request.user = user
        request.workspaceId = request.raw.workspaceId

        const userId = user?.id

        // detect root requests and exit from the guard
        if (dashboardTargetType === null && userId) {
          return true
        }
        const targetId = dashboardTargetType === 'workspace' ? request.raw.workspaceId : request.raw.projectId

        if (!targetId || !userId) {
          // false by default (will not allow to bypass)
          return optionalGuard
        }

        const hasAccess = await this.userService.hasMinimalAccessLevel({
          userId,
          targetId,
          targetType: dashboardTargetType,
          accessLevel: minimalRole,
          transaction: request.raw.transaction
        })

        if (hasAccess && dashboardTargetType === 'project') {
          request.projectId = targetId
        }

        return hasAccess
      } catch (e) {
        // false by default (will not allow to bypass)
        return optionalGuard
      }
    }

    return await checkDashboardAccess()
  }
}

export const AuthGuard = (
  dashboardTargetType: TDashboardTargetType | null = 'workspace',
  minimalRole: TUserRoles = USER_ROLE_EDITOR,
  optionalGuard: boolean = false
) =>
  applyDecorators(
    UseGuards(GlobalAuthGuard, ThrottleByTokenGuard),
    SetMetadata('dashboardTargetType', dashboardTargetType),
    SetMetadata('minimalRole', minimalRole),
    // false by default (will not allow to bypass)
    SetMetadata('optionalGuard', optionalGuard)
  )
