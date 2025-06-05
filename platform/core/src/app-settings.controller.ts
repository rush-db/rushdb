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
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'

@Controller('')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, NeogmaDataInterceptor)
export class AppSettingsController {
  constructor(
    private readonly configService: ConfigService,
    private readonly projectService: ProjectService
  ) {}

  @Get('settings')
  @ApiBearerAuth()
  @AuthGuard('project')
  async settings(@TransactionDecorator() transaction: Transaction, @Request() request: PlatformRequest) {
    const selfHosted = this.configService.get('RUSHDB_SELF_HOSTED')
    const dashboardUrl = this.configService.get('RUSHDB_DASHBOARD_URL')
    const googleAuthClientId = this.configService.get('GOOGLE_CLIENT_ID')
    const githubAuthClientId = this.configService.get('GH_CLIENT_ID')
    const projectId = request.projectId

    const project = await this.projectService.getProject(projectId, transaction)

    return {
      selfHosted: toBoolean(selfHosted),
      dashboardUrl: dashboardUrl,
      googleOAuthEnabled: toBoolean(googleAuthClientId),
      githubOAuthEnabled: toBoolean(githubAuthClientId),
      customDb: toBoolean(project.toJson().customDb)
    }
  }
}
