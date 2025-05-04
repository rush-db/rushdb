import { TUserProperties } from '../model/user.interface'

type TIUserAuthProperties = 'googleAuth' | 'githubAuth' | 'password'

export type AcceptWorkspaceInvitationParams<T extends boolean = boolean> =
  T extends true ?
    {
      inviteToken: string
      forceUserSignUp: true
      userData: Omit<TUserProperties, 'id' | 'isEmail'>
    }
  : {
      inviteToken: string
      forceUserSignUp?: false
    }
export interface IUserProperties extends Omit<TUserProperties, TIUserAuthProperties> {}
