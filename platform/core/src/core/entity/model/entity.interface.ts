import { ModelRelatedNodesI, NeogmaInstance, NeogmaModel } from 'neogma'

import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_LABEL,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META
} from '@/core/common/constants'
import { TPropertyFactory, TPropertyInstance } from '@/core/property/model/property.interface'
import { TPropertySingleValue, TPropertyValue } from '@/core/property/property.types'

type TEntityPropertiesNormalized = {
  [RUSHDB_KEY_ID]: string
  [RUSHDB_KEY_PROJECT_ID]: string
  [RUSHDB_KEY_LABEL]?: string
  [RUSHDB_KEY_PROPERTIES_META]?: string
} & Partial<{
  [key: string]: TPropertyValue
}>

type TEntityProperties = {
  [RUSHDB_KEY_ID]: string
  [RUSHDB_KEY_PROJECT_ID]: string
}

interface IEntitiesRelatedNodes {
  RelatedEntities: ModelRelatedNodesI<
    TEntityModel,
    TEntityInstance,
    { Metadata: string },
    { metadata: string }
  >
  Properties: ModelRelatedNodesI<
    TPropertyFactory,
    TPropertyInstance,
    {
      Metadata: string
    },
    {
      metadata: string
    }
  >
}

type TEntityInstance = NeogmaInstance<TEntityProperties, IEntitiesRelatedNodes>

type TEntityModel = NeogmaModel<TEntityProperties, IEntitiesRelatedNodes>

export {
  TEntityInstance,
  TEntityProperties,
  IEntitiesRelatedNodes,
  TEntityModel,
  TEntityPropertiesNormalized
}
