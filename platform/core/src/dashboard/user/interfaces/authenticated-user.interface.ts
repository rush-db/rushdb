import { User } from '@/dashboard/user/user.entity'

import { IUserProperties } from './user-properties.interface'
import { TUserRoles } from '@/dashboard/user/model/user.interface'

export interface IAuthenticatedUser extends IUserProperties {
  token: string
}

export interface IAuthenticatedUserWithAccess extends IAuthenticatedUser {
  currentScope: {
    role: TUserRoles
  }
}

export interface ICreatedUserData {
  userData: User
}
