import { TUserProperties } from '../model/user.interface'

type TIUserAuthProperties = 'googleAuth' | 'githubAuth' | 'password'

export type AcceptWorkspaceInvitationParams = {
  inviteToken: string
  authUserLogin: string
}
export interface IUserProperties extends Omit<TUserProperties, TIUserAuthProperties> {}
