import { Injectable } from '@nestjs/common'

import { TUserFactory } from '@/dashboard/user/model/user.interface'
import { User } from '@/dashboard/user/model/user.model'
import { RepositoryService } from '@/database/neogma/repository/repository.service'

@Injectable()
export class UserRepository {
  constructor(private readonly repositoryService: RepositoryService) {}

  get model() {
    return this.repositoryService.getModelByConfig<TUserFactory>(User)
  }
}
