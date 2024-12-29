import { User } from '@/dashboard/user/user.entity'

import { IUserProperties } from './user-properties.interface'

export interface IAuthenticatedUser extends IUserProperties {
  token: string
}

export interface ICreatedUserData {
  userData: User
}
