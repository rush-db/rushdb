import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import type { ProjectToken } from '~/features/tokens/types'

import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'
import { trackApiKeyGenerated } from '~/lib/analytics'
import { $currentProjectId } from '~/features/projects/stores/id'
import { $currentWorkspaceId } from '~/features/workspaces/stores/current'

export const useAddTokenMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: (args: Parameters<typeof api.tokens.create>[0]) => api.tokens.create(args),
    onSuccess() {
      trackApiKeyGenerated()
      if (projectId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.projects.tokens(projectId) })
      }
    }
  })
}

export const useDeleteTokenMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: ({ tokenId }: { tokenId: ProjectToken['id'] }) => api.tokens.delete({ id: tokenId }),
    onSuccess() {
      if (projectId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.projects.tokens(projectId) })
      }
    }
  })
}

// Workspace-level variants — list every key across the workspace's projects, and
// create/delete keys against an explicitly chosen project, invalidating the
// workspace listing on success.

export const useWorkspaceTokensQuery = () => {
  const workspaceId = useStore($currentWorkspaceId)
  return useQuery({
    queryKey: queryKeys.workspaces.tokens(workspaceId!),
    queryFn: () => api.tokens.listAll(),
    enabled: !!workspaceId
  })
}

export const useAddWorkspaceTokenMutation = () => {
  const queryClient = useQueryClient()
  const workspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: (args: Parameters<typeof api.tokens.create>[0]) => api.tokens.create(args),
    onSuccess(_data, variables) {
      trackApiKeyGenerated()
      if (workspaceId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.tokens(workspaceId) })
      }
      if (variables.projectId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.projects.tokens(variables.projectId) })
      }
    }
  })
}

export const useDeleteWorkspaceTokenMutation = () => {
  const queryClient = useQueryClient()
  const workspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: ({ tokenId, projectId }: { tokenId: ProjectToken['id']; projectId: string }) =>
      api.tokens.delete({ id: tokenId, projectId }),
    onSuccess(_data, variables) {
      if (workspaceId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.tokens(workspaceId) })
      }
      queryClient.invalidateQueries({ queryKey: queryKeys.projects.tokens(variables.projectId) })
    }
  })
}
