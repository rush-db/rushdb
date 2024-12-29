import { Injectable } from '@nestjs/common'

import { Property } from '@/core/property/model/property.model'
import { RepositoryService } from '@/database/neogma/repository/repository.service'

import { TPropertyFactory } from './property.interface'

@Injectable()
export class PropertyRepository {
  constructor(private readonly repositoryService: RepositoryService) {}

  get model() {
    return this.repositoryService.getModelByConfig<TPropertyFactory>(Property)
  }
}
