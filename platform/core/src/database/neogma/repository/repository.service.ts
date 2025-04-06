import { Injectable } from '@nestjs/common'
import { ModelFactory } from 'neogma'
import { NeogmaModel, RelationshipsI } from 'neogma/dist/ModelOps/ModelOps'

import { TAnyObject, TGetFirstArgument } from '@/common/types/utils'
import { TModelName, TModelSourceType } from '@/database/neogma/repository/types'
import { CompositeNeogmaService } from '@/database/neogma-dynamic/composite-neogma.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
export class RepositoryService {
  constructor(
    private readonly compositeNeogmaService: CompositeNeogmaService,
    private readonly neogmaService: NeogmaService
  ) {}

  private storage: Record<string, NeogmaModel<any, any>> = {}

  private relationshipCreationQuery: Record<string, Partial<RelationshipsI<TAnyObject>>> = {}

  public getModelByToken(token: string) {
    return this.storage[token]
  }

  public getModelByConfig<TModelFactory>(model: TGetFirstArgument<typeof ModelFactory> & TModelName) {
    return this.storage[model.name] as unknown as TModelFactory
  }

  public init<T extends TGetFirstArgument<typeof ModelFactory> & TModelName & TModelSourceType>(
    models: Array<T> = []
  ) {
    models.forEach((model) => {
      const { relationships, ...rest } = model

      const Model = ModelFactory<any, any>(
        rest,
        model.canUseExternalSource ?
          this.compositeNeogmaService.getInstance()
        : this.neogmaService.getInstance()
      )

      if (relationships) {
        this.relationshipCreationQuery[model.name] = relationships
      }
      this.storage[model.name] = Model
    })
    this.initRelationships()
  }

  private initRelationships() {
    Object.entries(this.relationshipCreationQuery).forEach(([modelName, relationShips]) => {
      this.storage[modelName].addRelationships(
        Object.keys(relationShips).reduce((acc, relationshipKey) => {
          if (relationShips[relationshipKey].model === 'self') {
            return {
              ...acc,
              [relationshipKey]: { ...relationShips[relationshipKey] }
            }
          }
          return {
            ...acc,
            [relationshipKey]: {
              ...relationShips[relationshipKey],
              model: this.getModelByToken(relationShips[relationshipKey].model)
            }
          }
        }, {})
      )
    })
  }
}
