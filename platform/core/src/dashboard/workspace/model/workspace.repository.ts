import { Injectable } from '@nestjs/common'

import { TWorkspaceModel } from '@/dashboard/workspace/model/workspace.interface'
import { Workspace } from '@/dashboard/workspace/model/workspace.model'
import { RepositoryService } from '@/database/neogma/repository/repository.service'

@Injectable()
export class WorkspaceRepository {
  constructor(private readonly repositoryService: RepositoryService) {}

  get model() {
    return this.repositoryService.getModelByConfig<TWorkspaceModel>(Workspace)
  }
}
