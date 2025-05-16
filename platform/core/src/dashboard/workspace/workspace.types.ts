import { TUserRoles } from '@/dashboard/user/model/user.interface'
import { TWorkspaceProperties } from '@/dashboard/workspace/model/workspace.interface'

export type TWorkSpaceInviteToken = {
  workspaceId: string
  email: string
  projectIds?: string[]
}

export type TWorkspaceInvitation = TWorkSpaceInviteToken & {
  workspaceName: string
  senderEmail: string
}

export type TExtendedWorkspaceProperties = TWorkspaceProperties & {
  role: TUserRoles
}

export type TNormalizedPendingInvite = {
  email: string
  createdAt: string
}
