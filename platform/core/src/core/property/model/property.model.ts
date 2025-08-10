import { ModelFactory } from 'neogma'

import { TGetFirstArgument } from '@/common/types/utils'
import { RUSHDB_LABEL_PROPERTY, RUSHDB_RELATION_VALUE } from '@/core/common/constants'
import {
  IPropertiesRelatedNodes,
  IPropertyStatics,
  TPropertyProperties
} from '@/core/property/model/property.interface'
import { TModelName, TModelSourceType } from '@/database/neogma/repository/types'

type TPropertyParams = TGetFirstArgument<
  typeof ModelFactory<TPropertyProperties, IPropertiesRelatedNodes, IPropertyStatics>
> &
  TModelName &
  TModelSourceType

export const Property: TPropertyParams = {
  name: 'Property',
  canUseExternalSource: true,
  label: RUSHDB_LABEL_PROPERTY,
  schema: {
    id: {
      type: 'string',
      required: true
    },
    name: {
      type: 'string',
      required: true
    },
    type: {
      type: 'string',
      required: true
    },
    projectId: {
      type: 'string'
    },
    metadata: {
      type: 'string'
    }
  },
  relationships: {
    Entities: {
      model: 'Entity',
      direction: 'out',
      name: RUSHDB_RELATION_VALUE
    }
  },
  primaryKeyField: 'id',
  statics: {}
}
