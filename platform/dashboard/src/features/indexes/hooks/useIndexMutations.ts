import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import type { EmbeddingIndex } from '~/features/indexes/types'

import { toast } from '~/elements/Toast'
import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'
import { $currentProjectId } from '~/features/projects/stores/id'

export const useAddIndexMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: (args: Parameters<typeof api.indexes.create>[0]) => api.indexes.create(args),
    onSuccess() {
      if (projectId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.projects.indexes(projectId) })
      }
    },
    onError(error) {
      toast({
        title: 'Failed to create index',
        description: error instanceof Error ? error.message : 'An unexpected error occurred',
        variant: 'danger'
      })
    }
  })
}

export const useDeleteIndexMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: ({ indexId }: { indexId: EmbeddingIndex['id'] }) => api.indexes.delete({ id: indexId }),
    onMutate: async ({ indexId }) => {
      if (!projectId) {
        return { previousIndexes: undefined as EmbeddingIndex[] | undefined }
      }

      const queryKey = queryKeys.projects.indexes(projectId)
      await queryClient.cancelQueries({ queryKey })

      const previousIndexes = queryClient.getQueryData<EmbeddingIndex[]>(queryKey)

      queryClient.setQueryData<EmbeddingIndex[]>(queryKey, (current) =>
        (current ?? []).filter((index) => index.id !== indexId)
      )

      return { previousIndexes }
    },
    onSuccess() {
      toast({
        title: 'Index deleted successfully'
      })
    },
    onError(error, _variables, context) {
      if (projectId && context?.previousIndexes) {
        queryClient.setQueryData(queryKeys.projects.indexes(projectId), context.previousIndexes)
      }
      toast({
        title: 'Failed to delete index',
        description: error instanceof Error ? error.message : 'An unexpected error occurred',
        variant: 'danger'
      })
    },
    onSettled() {
      if (projectId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.projects.indexes(projectId) })
      }
    }
  })
}
