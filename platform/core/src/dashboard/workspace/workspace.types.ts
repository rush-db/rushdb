import { TWorkspaceProperties } from '@/dashboard/workspace/model/workspace.interface'
import { TUserRoles } from '@/dashboard/user/model/user.interface'

export type TWorkSpaceInviteToken = {
  workspaceId: string
  email: string
  projectIds?: string[]
  isUserRegistered?: boolean
}

export type TWorkspaceInvitation = TWorkSpaceInviteToken & {
  workspaceName: string
  senderEmail: string
}

export type TExtendedWorkspaceProperties = TWorkspaceProperties & {
  role: TUserRoles
}
