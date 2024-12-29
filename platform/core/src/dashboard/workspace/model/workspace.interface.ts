import { ModelRelatedNodesI, NeogmaInstance, NeogmaModel } from 'neogma'

import { TProjectInstance, TProjectModel } from '@/dashboard/project/model/project.interface'
import { TUserFactory, TUserInstance } from '@/dashboard/user/model/user.interface'

type TWorkspaceLimits = {
  records: number
  importSize: number
  projects?: number
}

type TWorkspaceProperties = {
  id: string
  name: string
  created: string
  edited?: string
  s3Url?: string
  s3Bucket?: string
  s3User?: string
  s3Password?: string

  // @FYI: Andrew - those below are billing related fields:
  limits: string
  stats?: string
  validTill?: string
  planId?: string
  isSubscriptionCancelled?: boolean
}

interface IWorkspaceProperties extends TWorkspaceProperties {}

interface IWorkspaceRelatedNodes {
  Users: ModelRelatedNodesI<
    TUserFactory,
    TUserInstance,
    {
      Since: string
      Role: string
    },
    {
      since: string
      role: string
    }
  >
  Projects: ModelRelatedNodesI<TProjectModel, TProjectInstance>
}

// eslint-disable-next-line @typescript-eslint/no-empty-interface
export interface IWorkspaceStatics {}

type TWorkspaceInstance = NeogmaInstance<TWorkspaceProperties, IWorkspaceRelatedNodes>

type TWorkspaceModel = NeogmaModel<TWorkspaceProperties, IWorkspaceRelatedNodes, IWorkspaceStatics>

export {
  TWorkspaceInstance,
  TWorkspaceProperties,
  IWorkspaceRelatedNodes,
  TWorkspaceModel,
  IWorkspaceProperties,
  TWorkspaceLimits
}
