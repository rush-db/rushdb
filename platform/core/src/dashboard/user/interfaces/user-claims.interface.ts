import { TUserProperties } from '../model/user.interface'

type TIUserClaims = 'id'

export interface IUserClaims extends Pick<TUserProperties, TIUserClaims> {}
