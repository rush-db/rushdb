import { TUserProperties } from '../model/user.interface'

type TIUserClaims = 'id'

// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface IUserClaims extends Pick<TUserProperties, TIUserClaims> {}
