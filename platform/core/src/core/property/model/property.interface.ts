import { ModelRelatedNodesI, NeogmaInstance } from 'neogma'
import { NeogmaModel } from 'neogma/dist/ModelOps/ModelOps'

import { TEntityInstance, TEntityModel } from '@/core/entity/model/entity.interface'
import { TPropertySingleValue, TPropertyType, TPropertyValue } from '@/core/property/property.types'

type TPropertyPropertiesNormalized = TPropertyProperties & {
  value: TPropertyValue
}

type TPropertyProperties = {
  id: string
  name: string
  type: TPropertyType
  projectId: string
  metadata?: string
}

interface IPropertiesRelatedNodes {
  Entities: ModelRelatedNodesI<
    TEntityModel,
    TEntityInstance,
    {
      Value: TPropertySingleValue
      Metadata: string
    },
    {
      value: TPropertySingleValue
      metadata: number
    }
  >
}
// eslint-disable-next-line @typescript-eslint/no-empty-interface
interface IPropertyStatics {}

type TPropertyInstance = NeogmaInstance<TPropertyProperties, IPropertiesRelatedNodes>

type TPropertyFactory = NeogmaModel<TPropertyProperties, IPropertiesRelatedNodes>

export {
  TPropertyInstance,
  TPropertyFactory,
  TPropertyProperties,
  IPropertiesRelatedNodes,
  IPropertyStatics,
  TPropertyPropertiesNormalized
}
