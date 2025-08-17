import { forwardRef, Module } from '@nestjs/common'

import { EntityModule } from '@/core/entity/entity.module'
import { BillingModule } from '@/dashboard/billing/billing.module'
import { ProjectModule } from '@/dashboard/project/project.module'
import { TokenModule } from '@/dashboard/token/token.module'
import { WorkspaceRepository } from '@/dashboard/workspace/model/workspace.repository'
import { WorkspaceQueryService } from '@/dashboard/workspace/workspace-query.service'
import { WorkspaceController } from '@/dashboard/workspace/workspace.controller'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'

@Module({
  imports: [
    forwardRef(() => ProjectModule),
    forwardRef(() => BillingModule),
    forwardRef(() => TokenModule),
    forwardRef(() => EntityModule)
  ],
  providers: [WorkspaceRepository, WorkspaceService, WorkspaceQueryService],
  exports: [WorkspaceRepository, WorkspaceService, WorkspaceQueryService],
  controllers: [WorkspaceController]
})
export class WorkspaceModule {}
