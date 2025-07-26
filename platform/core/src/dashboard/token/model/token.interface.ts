import { ModelRelatedNodesI, NeogmaInstance, NeogmaModel } from 'neogma'

import { TProjectInstance, TProjectModel } from '@/dashboard/project/model/project.interface'
import { READ_ACCESS, WRITE_ACCESS } from '@/dashboard/token/token.constants'

type TTokenProperties = {
  id: string
  name: string
  created: string
  expiration: number
  description?: string
  value?: string
  prefixValue?: string
}

export type TAccessLevel = typeof WRITE_ACCESS | typeof READ_ACCESS

interface ITokenProperties extends TTokenProperties {}

interface ITokenRelatedNodes {
  Projects: ModelRelatedNodesI<
    TProjectModel,
    TProjectInstance,
    {
      Level: string
    },
    {
      level: string
    }
  >
}

// eslint-disable-next-line @typescript-eslint/no-empty-interface
export interface ITokenStatics {}

type TTokenInstance = NeogmaInstance<TTokenProperties, ITokenRelatedNodes>

type TTokenModel = NeogmaModel<TTokenProperties, ITokenRelatedNodes, ITokenStatics>

export { TTokenInstance, TTokenProperties, ITokenRelatedNodes, TTokenModel, ITokenProperties }
