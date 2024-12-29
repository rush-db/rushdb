import { Injectable } from '@nestjs/common'

import { TProjectModel } from '@/dashboard/project/model/project.interface'
import { Project } from '@/dashboard/project/model/project.model'
import { RepositoryService } from '@/database/neogma/repository/repository.service'

@Injectable()
export class ProjectRepository {
  constructor(private readonly repositoryService: RepositoryService) {}

  get model() {
    return this.repositoryService.getModelByConfig<TProjectModel>(Project)
  }
}
