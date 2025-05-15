import { User } from '@/dashboard/user/user.entity'

import { IUserProperties } from './user-properties.interface'
import { TUserRoles } from '@/dashboard/user/model/user.interface'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'

export interface IAuthenticatedUser extends IUserProperties {
  token: string
}

export interface IAuthenticatedUserWithAccess extends IAuthenticatedUser {
  currentScope: {
    role: TUserRoles
  }
}

export type TShortUserDataWithRole = Pick<IUserClaims, 'id' | 'login'> & {
  role: TUserRoles
}

export interface ICreatedUserData {
  userData: User
}
