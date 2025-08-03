import { ModelRelatedNodesI, NeogmaInstance, NeogmaModel } from 'neogma'

import { TProjectCustomDbPayload } from '@/dashboard/project/project.types'
import { TUserFactory, TUserInstance } from '@/dashboard/user/model/user.interface'
import { TWorkspaceInstance, TWorkspaceModel } from '@/dashboard/workspace/model/workspace.interface'

type TProjectProperties = {
  id: string
  name: string
  created: string
  edited?: string
  deleted?: string
  description?: string
  stats?: string
  customDb?: string
  managedDb?: boolean
  managedDbPassword?: string
  validTill?: string
  planId?: string
  productId?: string
  priceId?: string
  isSubscriptionCancelled?: boolean
}

interface IProjectProperties extends TProjectProperties {}

interface IRawProjectProperties extends Omit<TProjectProperties, 'customDb'> {
  customDb: TProjectCustomDbPayload
}

interface IProjectRelatedNodes {
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
  Workspaces: ModelRelatedNodesI<TWorkspaceModel, TWorkspaceInstance>
}

// eslint-disable-next-line @typescript-eslint/no-empty-interface
export interface IProjectStatics {}

type TProjectInstance = NeogmaInstance<TProjectProperties, IProjectRelatedNodes>

type TProjectModel = NeogmaModel<TProjectProperties, IProjectRelatedNodes, IProjectStatics>

export {
  TProjectInstance,
  TProjectProperties,
  IProjectRelatedNodes,
  TProjectModel,
  IProjectProperties,
  IRawProjectProperties
}
