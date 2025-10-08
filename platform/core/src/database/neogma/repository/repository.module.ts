import { forwardRef, Module, OnApplicationBootstrap } from '@nestjs/common'

import { Project } from '@/dashboard/project/model/project.model'
import { Token } from '@/dashboard/token/model/token.model'
import { User } from '@/dashboard/user/model/user.model'
import { Workspace } from '@/dashboard/workspace/model/workspace.model'
import { NeogmaModule } from '@/database/neogma/neogma.module'
import { RepositoryService } from '@/database/neogma/repository/repository.service'

@Module({
  imports: [forwardRef(() => NeogmaModule)],
  providers: [RepositoryService],
  exports: [RepositoryService]
})
export class RepositoryModule implements OnApplicationBootstrap {
  constructor(private readonly repositoryService: RepositoryService) {}

  onApplicationBootstrap() {
    this.repositoryService.init([
      // DASHBOARD
      User,
      Workspace,
      Project,
      Token
    ])
  }
}
