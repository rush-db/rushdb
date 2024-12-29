import { ModelFactory } from 'neogma'

import { TGetFirstArgument } from '@/common/types/utils'
import {
  RUSHDB_LABEL_USER,
  RUSHDB_RELATION_HAS_ACCESS,
  RUSHDB_RELATION_MEMBER_OF
} from '@/dashboard/common/constants'
import { IUserRelatedNodes, IUserStatics, TUserProperties } from '@/dashboard/user/model/user.interface'
import { TModelName } from '@/database/neogma/repository/types'

type TPropertyParams = TGetFirstArgument<
  typeof ModelFactory<TUserProperties, IUserRelatedNodes, IUserStatics>
> &
  TModelName

export const User: TPropertyParams = {
  name: 'User',
  label: RUSHDB_LABEL_USER,
  schema: {
    id: {
      type: 'string',
      required: true
    },
    login: {
      type: 'string',
      required: true
    },
    isEmail: {
      type: 'boolean',
      required: true
    },
    firstName: {
      type: 'string',
      required: false
    },
    lastName: {
      type: 'string',
      required: false
    },
    confirmed: {
      type: 'boolean',
      required: true
    },
    status: {
      type: 'string',
      required: false
    },
    created: {
      type: 'string',
      required: true
    },
    settings: {
      type: 'string',
      required: false
    },
    edited: {
      type: 'string',
      required: false
    },
    lastActivity: {
      type: 'string',
      required: false
    },
    googleAuth: {
      type: 'string',
      required: false
    },
    githubAuth: {
      type: 'string',
      required: false
    },
    password: {
      type: 'string',
      required: false
    },
    deletedDate: {
      type: 'string',
      required: false
    }
  },
  relationships: {
    Workspaces: {
      model: 'Workspace',
      direction: 'out',
      name: RUSHDB_RELATION_MEMBER_OF,
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
    Projects: {
      model: 'Project',
      direction: 'out',
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
    }
  },
  primaryKeyField: 'id',
  statics: {}
}
