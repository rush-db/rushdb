import { Controller, Get, UseInterceptors } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { toBoolean } from '@/common/utils/toBolean'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'

@Controller('')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, NeogmaDataInterceptor)
export class AppController {
  constructor(private readonly configService: ConfigService) {}

  @Get('')
  root(): string {
    return 'It works!'
  }

  @Get('settings')
  settings() {
    const selfHosted = this.configService.get('RUSHDB_SELF_HOSTED')
    const dashboardUrl = this.configService.get('RUSHDB_DASHBOARD_URL')

    return {
      selfHosted: toBoolean(selfHosted),
      dashboardUrl: dashboardUrl
    }
  }
}