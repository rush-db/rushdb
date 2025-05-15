import type { PlanId } from '~/features/billing/types'
import type { ISO8601 } from '~/types'
import { GetUserResponse } from '~/features/auth/types.ts'

export type Workspace = {
  created: ISO8601
  id: string
  limits: {
    projects: number
    users?: number
  }
  name: string
  planId?: PlanId
  validTill?: ISO8601
  isSubscriptionCancelled?: boolean
}

export interface WorkspaceUser {
  id: string
  login: string
  role: string
}

export interface WorkspaceAccessList {
  [projectId: string]: string[]
}

export interface InviteToWorkspaceDto {
  email: string
  projectIds?: string[]
}

export interface AcceptedUserInviteDto {
  userData: GetUserResponse
  workspaceId: string
}

export interface RevokeAccessDto {
  userIds: string[]
}

export interface PendingInvite {
  email: string
  createdAt: string
}
