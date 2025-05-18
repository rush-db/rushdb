import type { GenericApiResponse, ISO8601 } from '~/types'

export type AuthorizedUser = {
  created: ISO8601
  login: string
  confirmed: boolean
  id: string
  settings: string
  token: string
  isEmail?: boolean
  firstName?: string
  lastName?: string
  currentScope?: {
    role: 'owner' | 'developer'
  }
}

export type GetUserResponse = GenericApiResponse<Omit<AuthorizedUser, 'settings'> & { settings: string }>
export type InvitedGetUserResponse = GenericApiResponse<
  Omit<AuthorizedUser, 'settings'> & { settings: string } & { workspaceId?: string }
>

export type User = { isLoggedIn: boolean } & Partial<AuthorizedUser>
