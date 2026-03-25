import { forwardRef, Module } from '@nestjs/common'

import { EntityModule } from '@/core/entity/entity.module'
import { PropertyModule } from '@/core/property/property.module'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'
import { ProjectQueryService } from '@/dashboard/project/project-query.service'
import { ProjectController } from '@/dashboard/project/project.controller'
import { ProjectService } from '@/dashboard/project/project.service'
import { TokenModule } from '@/dashboard/token/token.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'
import { NeogmaDynamicModule } from '@/database/neogma-dynamic/neogma-dynamic.module'

@Module({
  imports: [
    forwardRef(() => WorkspaceModule),
    forwardRef(() => EntityModule),
    forwardRef(() => PropertyModule),
    forwardRef(() => TokenModule),

    //db modules
    forwardRef(() => NeogmaDynamicModule)
  ],
  providers: [ProjectRepository, ProjectService, ProjectQueryService],
  exports: [ProjectRepository, ProjectService, ProjectQueryService],
  controllers: [ProjectController]
})
export class ProjectModule {}
