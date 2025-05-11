import { TUserProperties } from '../model/user.interface'

type TIUserAuthProperties = 'googleAuth' | 'githubAuth' | 'password'

export type AcceptWorkspaceInvitationParams<T extends boolean = boolean> =
  T extends true ?
    {
      forceUserSignUp: true
      inviteToken: string
      userData: Omit<TUserProperties, 'id' | 'isEmail'>
    }
  : {
      forceUserSignUp?: false
      inviteToken: string
      authUserLogin: string
    }
export interface IUserProperties extends Omit<TUserProperties, TIUserAuthProperties> {}
