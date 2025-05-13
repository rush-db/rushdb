import { createMutator } from '~/lib/fetcher'
import { api } from '~/lib/api'
import { $currentWorkspaceId } from './current'
import { setCurrentWorkspace } from './current-workspace'
import { type ApiParams } from '~/lib/api'
import { getRoutePath, openRoute } from '~/lib/router'
import { toast } from '~/elements/Toast'

// Mutator for accepting workspace invitations
export const acceptInvitation = createMutator({
  async fetcher({ token, init }: ApiParams<typeof api.workspaces.acceptInvitation>) {
    return await api.workspaces.acceptInvitation({ token, init })
  },
  onSuccess(workspace) {
    if (workspace && workspace.id) {
      // Set the new workspace as current
      setCurrentWorkspace(workspace.id)

      // Show success toast
      toast({
        title: 'Invitation accepted',
        description: `You've been added to ${workspace.name} workspace`,
        duration: 5000
      })

      // Redirect to home page
      openRoute('home')
    }
  },
  onError(error) {
    // Show error toast
    toast({
      title: 'Error accepting invitation',
      description: error instanceof Error ? error.message : 'Failed to accept workspace invitation',
      variant: 'danger',
      duration: 5000
    })
  }
})

// Extract invite token from URL
export function getInviteTokenFromURL(): string | null {
  const url = new URL(window.location.href)
  // Check for both possible query parameter names
  return url.searchParams.get('invite') || url.searchParams.get('token')
}
