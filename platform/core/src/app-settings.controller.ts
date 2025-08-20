import { Controller, Get, Request, UseInterceptors } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { ApiBearerAuth } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { toBoolean } from '@/common/utils/toBolean'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { ProjectService } from '@/dashboard/project/project.service'
import { USER_ROLE_EDITOR } from '@/dashboard/user/interfaces/user.constants'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { TransactionDecorator } from '@/database/transaction.decorator'

@Controller('')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, DataInterceptor)
export class AppSettingsController {
  constructor(
    private readonly configService: ConfigService,
    private readonly projectService: ProjectService
  ) {}

  commonSettings() {
    const selfHosted = this.configService.get('RUSHDB_SELF_HOSTED')
    const dashboardUrl = this.configService.get('RUSHDB_DASHBOARD_URL')
    const googleAuthClientId = this.configService.get('GOOGLE_CLIENT_ID')
    const githubAuthClientId = this.configService.get('GH_CLIENT_ID')
    return {
      selfHosted: toBoolean(selfHosted),
      dashboardUrl: dashboardUrl,
      googleOAuthEnabled: toBoolean(googleAuthClientId),
      githubOAuthEnabled: toBoolean(githubAuthClientId)
    }
  }

  @Get('settings')
  settings() {
    return this.commonSettings()
  }

  @Get('sdk/settings')
  @ApiBearerAuth()
  @AuthGuard('project', USER_ROLE_EDITOR, true)
  /* @deprecated */
  async sdkSettings(@TransactionDecorator() transaction: Transaction, @Request() request: PlatformRequest) {
    const projectId = request.projectId
    let customDb = undefined

    if (projectId) {
      const project = await this.projectService.getProject(projectId, transaction)
      customDb = toBoolean(project.toJson().customDb)
    }
    const commonSettings = this.commonSettings()

    return {
      ...commonSettings,
      customDb
    }
  }
}
