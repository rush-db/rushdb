import { ModelFactory } from 'neogma'

import { TGetFirstArgument } from '@/common/types/utils'
import { RUSHDB_LABEL_TOKEN, RUSHDB_RELATION_HAS_ACCESS } from '@/dashboard/common/constants'
import { ITokenRelatedNodes, ITokenStatics, TTokenProperties } from '@/dashboard/token/model/token.interface'
import { TModelName } from '@/database/neogma/repository/types'

type TTokenParams = TGetFirstArgument<
  typeof ModelFactory<TTokenProperties, ITokenRelatedNodes, ITokenStatics>
> &
  TModelName

export const Token: TTokenParams = {
  name: 'Token',
  label: RUSHDB_LABEL_TOKEN,
  schema: {
    id: {
      type: 'string',
      required: true
    },
    name: {
      type: 'string',
      required: true
    },
    expiration: {
      type: 'number',
      required: true
    },
    created: {
      type: 'string',
      required: true
    },
    description: {
      type: 'string'
    },
    value: {
      type: 'string',
      required: true
    }
  },
  primaryKeyField: 'id',
  statics: {},
  relationships: {
    Projects: {
      model: 'Project',
      direction: 'out',
      name: RUSHDB_RELATION_HAS_ACCESS,
      properties: {
        Level: {
          property: 'level',
          schema: {
            type: 'string'
          }
        }
      }
    }
  }
}
