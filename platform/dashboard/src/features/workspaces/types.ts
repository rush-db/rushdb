import type { PlanId } from '~/features/billing/types'
import type { ISO8601 } from '~/types'
import type { GetUserResponse } from '~/features/auth/types.ts'

export type WorkspaceRole = 'owner' | 'admin' | 'developer' | 'viewer'

export const WORKSPACE_ROLES: WorkspaceRole[] = ['owner', 'admin', 'developer', 'viewer']

export type Workspace = {
  created: ISO8601
  id: string
  name: string
  planId?: PlanId
  validTill?: ISO8601
  role?: WorkspaceRole
  isSubscriptionCancelled?: boolean
  // Operational limits from billing service (injected at runtime)
  projectLimit?: number | null // null = unlimited
  userLimit?: number | null // null = unlimited
}

export interface WorkspaceUser {
  id: string
  login: string
  role: WorkspaceRole
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
