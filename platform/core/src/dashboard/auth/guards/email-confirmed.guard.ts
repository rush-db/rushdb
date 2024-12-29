import { CanActivate, ExecutionContext, Injectable, UnauthorizedException } from '@nestjs/common'
import { FastifyRequest } from 'fastify'

import { TUserProperties } from '@/dashboard/user/model/user.interface'

@Injectable()
export class EmailConfirmationGuard implements CanActivate {
  canActivate(context: ExecutionContext) {
    const request: FastifyRequest & { user: TUserProperties } = context.switchToHttp().getRequest()

    if (!request.user?.confirmed) {
      throw new UnauthorizedException('Confirm your email first')
    }

    return true
  }
}
