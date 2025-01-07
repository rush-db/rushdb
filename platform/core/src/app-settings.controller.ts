import { Controller, Get, UseInterceptors } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { toBoolean } from '@/common/utils/toBolean'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'

@Controller('')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, NeogmaDataInterceptor)
export class AppSettingsController {
  constructor(private readonly configService: ConfigService) {}

  @Get('settings')
  settings() {
    const selfHosted = this.configService.get('RUSHDB_SELF_HOSTED')
    const dashboardUrl = this.configService.get('RUSHDB_DASHBOARD_URL')
    const googleAuthClientId = this.configService.get('GOOGLE_CLIENT_ID')
    const githubAuthClientId = this.configService.get('GITHUB_CLIENT_ID')

    return {
      selfHosted: toBoolean(selfHosted),
      dashboardUrl: dashboardUrl,
      googleOAuthEnabled: toBoolean(googleAuthClientId),
      githubOAuthEnabled: toBoolean(githubAuthClientId)
    }
  }
}
