import { forwardRef, Module } from '@nestjs/common'

import { BillingClientModule } from '@/core/billing-client/billing-client.module'
import { EntityModule } from '@/core/entity/entity.module'
import { BillingModule } from '@/dashboard/billing/billing.module'
import { OAuthRepository } from '@/dashboard/mcp-oauth/model/oauth.repository'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { WorkspaceRepository } from '@/dashboard/workspace/model/workspace.repository'
import { WorkspaceController } from '@/dashboard/workspace/workspace.controller'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'

@Module({
  imports: [
    BillingClientModule,
    forwardRef(() => ProjectModule),
    forwardRef(() => BillingModule),
    forwardRef(() => TokenModule),
    forwardRef(() => EntityModule)
  ],
  providers: [WorkspaceRepository, WorkspaceService, OAuthRepository],
  exports: [WorkspaceRepository, WorkspaceService],
  controllers: [WorkspaceController]
})
export class WorkspaceModule {}
