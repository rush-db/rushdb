import { Injectable } from '@nestjs/common'

import { TTokenModel } from '@/dashboard/token/model/token.interface'
import { Token } from '@/dashboard/token/model/token.model'
import { RepositoryService } from '@/database/neogma/repository/repository.service'

@Injectable()
export class TokenRepository {
  constructor(private readonly repositoryService: RepositoryService) {}

  get model() {
    return this.repositoryService.getModelByConfig<TTokenModel>(Token)
  }
}
