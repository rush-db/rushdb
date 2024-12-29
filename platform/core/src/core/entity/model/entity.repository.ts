import { Injectable } from '@nestjs/common'

import { Entity } from '@/core/entity/model/entity.model'
import { RepositoryService } from '@/database/neogma/repository/repository.service'

import { TEntityModel } from './entity.interface'

@Injectable()
export class EntityRepository {
  constructor(private readonly repositoryService: RepositoryService) {}

  get model() {
    return this.repositoryService.getModelByConfig<TEntityModel>(Entity)
  }
}
