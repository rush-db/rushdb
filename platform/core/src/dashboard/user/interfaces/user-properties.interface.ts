import { TUserProperties } from '../model/user.interface'

type TIUserAuthProperties = 'googleAuth' | 'githubAuth' | 'password'

export type AcceptWorkspaceInvitationParams = {
  inviteToken: string
  authUserLogin: string
}
export interface IUserProperties extends Omit<TUserProperties, TIUserAuthProperties> {
  /** Whether a Google account is linked as a sign-in method. Derived from googleAuth presence; the hash itself is never exposed. */
  googleConnected: boolean
  /** Whether a GitHub account is linked as a sign-in method. Derived from githubAuth presence; the hash itself is never exposed. */
  githubConnected: boolean
}
