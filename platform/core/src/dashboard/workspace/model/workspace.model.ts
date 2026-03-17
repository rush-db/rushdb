import { ModelFactory } from 'neogma'

import { TGetFirstArgument } from '@/common/types/utils'
import {
  RUSHDB_LABEL_WORKSPACE,
  RUSHDB_RELATION_CONTAINS,
  RUSHDB_RELATION_MEMBER_OF
} from '@/dashboard/common/constants'
import {
  IWorkspaceRelatedNodes,
  IWorkspaceStatics,
  TWorkspaceProperties
} from '@/dashboard/workspace/model/workspace.interface'
import { TModelName } from '@/database/neogma/repository/types'

type TWorkspaceParams = TGetFirstArgument<
  typeof ModelFactory<TWorkspaceProperties, IWorkspaceRelatedNodes, IWorkspaceStatics>
> &
  TModelName

export const Workspace: TWorkspaceParams = {
  name: 'Workspace',
  label: RUSHDB_LABEL_WORKSPACE,
  schema: {
    id: {
      type: 'string',
      required: true
    },
    name: {
      type: 'string',
      required: true
    },
    created: {
      type: 'string',
      required: true
    },
    edited: {
      type: 'string',
      required: false
    },
    stats: {
      type: 'string',
      required: false
    },
    pendingInvites: {
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
      name: RUSHDB_RELATION_MEMBER_OF,
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
    Projects: {
      model: 'Project',
      direction: 'out',
      name: RUSHDB_RELATION_CONTAINS
    }
  }
}
