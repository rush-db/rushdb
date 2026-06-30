import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import { $currentProjectId } from '~/features/projects/stores/id'
import type { CreateSavedQueryInput } from '~/features/saved-queries/types'
import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'

export const useSavedQueriesQuery = () => {
  const projectId = useStore($currentProjectId)
  return useQuery({
    queryKey: projectId ? queryKeys.projects.savedQueries(projectId) : ['projects', 'saved-queries'],
    queryFn: () => api.savedQueries.list({ projectId: projectId! }),
    enabled: !!projectId
  })
}

export const useSaveQueryMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: (body: CreateSavedQueryInput) => api.savedQueries.create({ ...body, projectId: projectId! }),
    onSuccess() {
      if (projectId) queryClient.invalidateQueries({ queryKey: queryKeys.projects.savedQueries(projectId) })
    }
  })
}

export const useUpdateSavedQueryMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: ({ id, ...body }: { id: string } & Partial<CreateSavedQueryInput>) =>
      api.savedQueries.update({ id, ...body, projectId: projectId! }),
    onSuccess() {
      if (projectId) queryClient.invalidateQueries({ queryKey: queryKeys.projects.savedQueries(projectId) })
    }
  })
}

export const useDeleteSavedQueryMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: (id: string) => api.savedQueries.delete({ id, projectId: projectId! }),
    onSuccess() {
      if (projectId) queryClient.invalidateQueries({ queryKey: queryKeys.projects.savedQueries(projectId) })
    }
  })
}
