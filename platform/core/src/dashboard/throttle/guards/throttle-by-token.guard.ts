import { Injectable, ExecutionContext } from '@nestjs/common'
import { ThrottlerGuard, ThrottlerException } from '@nestjs/throttler'

import { PlatformRequest } from '@/common/types/request'

@Injectable()
export class ThrottleByTokenGuard extends ThrottlerGuard {
  // we want to generate user by endpoint, to prevent a cycle creation of records or smth
  generateKey(context: ExecutionContext, suffix: string) {
    const request = context.switchToHttp().getRequest()
    const url = request.url
    return `${url}-${suffix}`
  }

  // tracker works for both dashboard and sdk tokens
  protected async getTracker(request: PlatformRequest): Promise<string> {
    if (request.headers['authorization']) {
      return request.headers['authorization'].split(' ')[1]
    }

    throw new ThrottlerException('Missing token')
  }

  // We add our guard to auth guard decorator app/src/dashboard/auth/guards/global-auth.guard.ts
  protected async handleRequest(context: ExecutionContext, limit: number, ttl: number): Promise<boolean> {
    // false by default (will not allow to bypass)
    const optionalGuard = this.reflector.get<boolean>('optionalGuard', context.getHandler()) ?? false

    try {
      const request = context.switchToHttp().getRequest()

      const token = await this.getTracker(request)

      if (!token) {
        // false by default (will not allow to bypass)
        return optionalGuard
      }

      const key = this.generateKey(context, token)

      const { totalHits } = await this.storageService.increment(key, ttl)
      if (totalHits > limit) {
        throw new ThrottlerException('Too many requests')
      }

      return true
    } catch (error) {
      // Keep Throttler doing its job
      if (error instanceof ThrottlerException && error.message === 'Too many requests') {
        return false
      }

      // false by default (will not allow to bypass)
      return optionalGuard
    }
  }
}
