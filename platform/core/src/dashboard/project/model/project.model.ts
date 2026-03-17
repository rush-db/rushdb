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
      type: 'string',
      required: false
    },
    created: {
      type: 'string',
      required: true
    },
    edited: {
      type: 'string',
      required: false
    },
    deleted: {
      type: 'string',
      required: false
    },
    stats: {
      type: 'string',
      required: false
    },
    customDb: {
      type: 'string',
      required: false
    },
    status: {
      type: 'string',
      required: false
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
            type: 'string',
            required: false
          }
        },
        Role: {
          property: 'role',
          schema: {
            type: 'string',
            required: false
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
