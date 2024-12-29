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

  // tracker works both for dashboard and sdk tokens
  protected async getTracker(request: PlatformRequest): Promise<string> {
    let token: string
    if (request.headers['authorization']) {
      token = request.headers['authorization'].split(' ')[1]
    } else if (request.headers['token']) {
      token = request.headers['token'] as string
    } else {
      throw new ThrottlerException('Missing token')
    }

    return token
  }

  // We add our guard to auth guard decorator app/src/dashboard/auth/guards/global-auth.guard.ts
  protected async handleRequest(context: ExecutionContext, limit: number, ttl: number): Promise<boolean> {
    const request = context.switchToHttp().getRequest()
    const token = await this.getTracker(request)

    if (!token) {
      return false
    }

    const key = this.generateKey(context, token)

    const { totalHits } = await this.storageService.increment(key, ttl)

    if (totalHits > limit) {
      throw new ThrottlerException('Too many requests')
    }

    return true
  }
}
