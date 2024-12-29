import { TUserProperties } from '../model/user.interface'

type TIUserClaims = 'login' | 'firstName' | 'lastName' | 'id' | 'confirmed'

export interface IUserClaims extends Pick<TUserProperties, TIUserClaims> {}
