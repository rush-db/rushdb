import { TUserProperties } from '../model/user.interface'

type TIUserAuthProperties = 'googleAuth' | 'githubAuth' | 'password'

export type AcceptWorkspaceInvitationParams = {
  inviteToken: string
  authUserLogin: string
}
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface IUserProperties extends Omit<TUserProperties, TIUserAuthProperties> {}
