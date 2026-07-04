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
  /** ID of the OAuth consent that caused this token to be issued, if applicable. */
  consentId?: string
  level?: TAccessLevel
}

export type TAccessLevel = typeof WRITE_ACCESS | typeof READ_ACCESS

// eslint-disable-next-line @typescript-eslint/no-empty-object-type
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

// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface ITokenStatics {}

type TTokenInstance = NeogmaInstance<TTokenProperties, ITokenRelatedNodes>

type TTokenModel = NeogmaModel<TTokenProperties, ITokenRelatedNodes, ITokenStatics>

export { TTokenInstance, TTokenProperties, ITokenRelatedNodes, TTokenModel, ITokenProperties }
