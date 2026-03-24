import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import type { ProjectToken } from '~/features/tokens/types'

import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'
import { $currentProjectId } from '~/features/projects/stores/id'

export const useAddTokenMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: (args: Parameters<typeof api.tokens.create>[0]) => api.tokens.create(args),
    onSuccess() {
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
