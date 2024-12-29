import { ModelFactory } from 'neogma'

import { TGetFirstArgument } from '@/common/types/utils'
import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_LABEL_RECORD,
  RUSHDB_RELATION_DEFAULT,
  RUSHDB_RELATION_VALUE
} from '@/core/common/constants'
import { IEntitiesRelatedNodes, TEntityProperties } from '@/core/entity/model/entity.interface'
import { TModelName } from '@/database/neogma/repository/types'

type TEntityParams = TGetFirstArgument<typeof ModelFactory<TEntityProperties, IEntitiesRelatedNodes>> &
  TModelName

export const Entity: TEntityParams = {
  name: 'Entity',
  label: RUSHDB_LABEL_RECORD,
  schema: {
    [RUSHDB_KEY_ID]: {
      type: 'string',
      required: true
    },
    [RUSHDB_KEY_PROJECT_ID]: {
      type: 'string',
      required: true
    }
  },
  primaryKeyField: RUSHDB_KEY_ID,
  relationships: {
    RelatedEntities: {
      model: 'self',
      direction: 'none',
      name: RUSHDB_RELATION_DEFAULT,
      properties: {
        Metadata: {
          property: 'metadata',
          schema: {
            type: 'string'
          }
        }
      }
    },
    Properties: {
      model: 'Property',
      direction: 'in',
      name: RUSHDB_RELATION_VALUE
    }
  }
}
