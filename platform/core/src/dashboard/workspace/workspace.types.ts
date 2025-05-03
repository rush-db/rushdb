export type TWorkSpaceInviteToken = {
  workspaceId: string
  email: string
  projectIds?: string[]
}

export type TWorkspaceInvitation = TWorkSpaceInviteToken & {
  workspaceName: string
  senderEmail: string
}
