import { TUserProperties } from '../model/user.interface'

type TIUserAuthProperties = 'googleAuth' | 'githubAuth' | 'password'

export interface IUserProperties extends Omit<TUserProperties, TIUserAuthProperties> {}
