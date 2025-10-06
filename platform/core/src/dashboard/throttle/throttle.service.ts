import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { ThrottlerOptionsFactory, ThrottlerModuleOptions } from '@nestjs/throttler'

import { toBoolean } from '@/common/utils/toBolean'

@Injectable()
export class ThrottleService implements ThrottlerOptionsFactory {
  constructor(private readonly configService: ConfigService) {}

  // @TODO: Make it sensitive to current plan
  createThrottlerOptions(): ThrottlerModuleOptions {
    const ttl = this.configService.get('RATE_LIMITER_TTL') || 1000
    const limit =
      toBoolean(this.configService.get('RUSHDB_SELF_HOSTED')) ? 1000 : (
        this.configService.get('RATE_LIMITER_REQUESTS_LIMIT') || 100
      )

    return {
      throttlers: [
        {
          ttl,
          limit
        }
      ]
    }
  }
}
