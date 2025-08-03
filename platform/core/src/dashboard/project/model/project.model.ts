import { ModelFactory } from 'neogma'

import { TGetFirstArgument } from '@/common/types/utils'
import {
  RUSHDB_LABEL_PROJECT,
  RUSHDB_RELATION_CONTAINS,
  RUSHDB_RELATION_HAS_ACCESS
} from '@/dashboard/common/constants'
import {
  IProjectRelatedNodes,
  IProjectStatics,
  TProjectProperties
} from '@/dashboard/project/model/project.interface'
import { TModelName } from '@/database/neogma/repository/types'

type TProjectParams = TGetFirstArgument<
  typeof ModelFactory<TProjectProperties, IProjectRelatedNodes, IProjectStatics>
> &
  TModelName

export const Project: TProjectParams = {
  name: 'Project',
  label: RUSHDB_LABEL_PROJECT,
  schema: {
    id: {
      type: 'string',
      required: true
    },
    name: {
      type: 'string',
      required: true
    },
    description: {
      type: 'string'
    },
    created: {
      type: 'string',
      required: true
    },
    edited: {
      type: 'string'
    },
    deleted: {
      type: 'string'
    },
    stats: {
      type: 'string'
    },
    customDb: {
      type: 'string'
    },
    managedDb: {
      type: 'boolean'
    },
    managedDbPassword: {
      type: 'string'
    },
    planId: {
      type: 'string'
    },
    productId: {
      type: 'string'
    },
    priceId: {
      type: 'string'
    },
    // limits: {
    //   type: 'string',
    //   required: true
    // },
    validTill: {
      type: 'string'
    },
    isSubscriptionCancelled: {
      type: 'boolean'
    }
  },
  primaryKeyField: 'id',
  statics: {},
  relationships: {
    Users: {
      model: 'User',
      direction: 'in',
      name: RUSHDB_RELATION_HAS_ACCESS,
      properties: {
        Since: {
          property: 'since',
          schema: {
            type: 'string'
          }
        },
        Role: {
          property: 'role',
          schema: {
            type: 'string'
          }
        }
      }
    },
    Workspaces: {
      model: 'Workspace',
      direction: 'in',
      name: RUSHDB_RELATION_CONTAINS
    }
  }
}
