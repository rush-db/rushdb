import { createParamDecorator, ExecutionContext } from '@nestjs/common'

import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'

export const AuthUser = createParamDecorator<unknown, ExecutionContext, IUserClaims>((data, ctx) => {
  const request = ctx.switchToHttp().getRequest()
  return request.user
})
