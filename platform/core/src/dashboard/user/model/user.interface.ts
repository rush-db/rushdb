import { ModelRelatedNodesI, NeogmaInstance } from 'neogma'
import { NeogmaModel } from 'neogma/dist/ModelOps/ModelOps'

import { TProjectInstance, TProjectModel } from '@/dashboard/project/model/project.interface'
import { TWorkspaceInstance, TWorkspaceModel } from '@/dashboard/workspace/model/workspace.interface'

import {
  USER_ROLE_EDITOR,
  USER_ROLE_OWNER,
  USER_STATUS_ACTIVE,
  USER_STATUS_BLOCKED,
  USER_STATUS_DEACTIVATED,
  USER_STATUS_DELETED
} from '../interfaces/user.constants'

export type TUserStatuses =
  | typeof USER_STATUS_ACTIVE
  | typeof USER_STATUS_BLOCKED
  | typeof USER_STATUS_DEACTIVATED
  | typeof USER_STATUS_DELETED

export type TUserRoles =
  // | typeof USER_ROLE_ADMIN
  typeof USER_ROLE_OWNER | typeof USER_ROLE_EDITOR
// | typeof USER_ROLE_VIEWER

export type TUserProperties = {
  id: string
  login: string
  isEmail: boolean
  firstName?: string
  lastName?: string
  confirmed?: boolean
  status?: TUserStatuses
  created?: string
  settings?: string
  edited?: string
  about?: string
  lastActivity?: string
  googleAuth?: string
  githubAuth?: string
  facebookAuth?: string
  password?: string
  isOnboardingFinished?: boolean
  deletedDate?: string
}

// eslint-disable-next-line @typescript-eslint/no-empty-interface
export interface IUserStatics {}

export type TUserInstance = NeogmaInstance<TUserProperties, IUserRelatedNodes, IUserStatics>

export type TUserFactory = NeogmaModel<TUserProperties, IUserRelatedNodes, IUserStatics>

export interface IUserRelatedNodes {
  Workspaces: ModelRelatedNodesI<
    TWorkspaceModel,
    TWorkspaceInstance,
    {
      Since: string
      Role: TUserRoles
    },
    {
      since: string
      role: TUserRoles
    }
  >
  Projects: ModelRelatedNodesI<
    TProjectModel,
    TProjectInstance,
    {
      Since: string
      Role: TUserRoles
    },
    {
      since: string
      role: TUserRoles
    }
  >
}
