import { forwardRef, Module, OnApplicationBootstrap } from '@nestjs/common'

import { Entity } from '@/core/entity/model/entity.model'
import { Property } from '@/core/property/model/property.model'
import { Project } from '@/dashboard/project/model/project.model'
import { Token } from '@/dashboard/token/model/token.model'
import { User } from '@/dashboard/user/model/user.model'
import { Workspace } from '@/dashboard/workspace/model/workspace.model'
import { RepositoryService } from '@/database/neogma/repository/repository.service'
import { NeogmaDynamicModule } from '@/database/neogma-dynamic/neogma-dynamic.module'

@Module({
  // Maybe move repository module to another dir...
  imports: [forwardRef(() => NeogmaDynamicModule)],
  providers: [RepositoryService],
  exports: [RepositoryService]
})
export class RepositoryModule implements OnApplicationBootstrap {
  constructor(private readonly repositoryService: RepositoryService) {}

  onApplicationBootstrap() {
    // @TODO: Refactor to on-demand approach
    this.repositoryService.init([
      // CORE
      Entity,
      Property,
      // DASHBOARD
      User,
      Workspace,
      Project,
      Token
    ])
  }
}
