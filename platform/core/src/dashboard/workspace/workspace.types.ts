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
