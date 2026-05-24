import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import { toast } from '~/elements/Toast'
import { $currentProjectId } from '~/features/projects/stores/id'
import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'

export const useRelationshipPatternsQuery = () => {
  const projectId = useStore($currentProjectId)
  return useQuery({
    queryKey: queryKeys.projects.relationshipPatterns(projectId!),
    queryFn: () => api.relationshipPatterns.list({ projectId: projectId! }),
    enabled: !!projectId,
    refetchInterval: (query) => {
      const status = query.state.data?.analysis?.status
      const hasPendingApply = query.state.data?.patterns.some(
        (pattern) => pattern.status === 'approved' && !pattern.lastAppliedAt && !pattern.lastError
      )
      return status === 'pending' || status === 'running' || hasPendingApply ? 5000 : false
    }
  })
}

export const useAnalyzeRelationshipPatternsMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: () => api.relationshipPatterns.analyze({ projectId: projectId! }),
    onSuccess() {
      if (projectId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.projects.relationshipPatterns(projectId) })
      }
      toast({ title: 'Relationship analysis queued' })
    }
  })
}

export const useApproveRelationshipPatternMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: (id: string) => api.relationshipPatterns.approve({ projectId: projectId!, id }),
    onSuccess() {
      if (projectId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.projects.relationshipPatterns(projectId) })
        queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'records'] })
        queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'record-relations'] })
        queryClient.invalidateQueries({ queryKey: ['records'] })
      }
      toast({ title: 'Relationship pattern approved' })
    }
  })
}

export const useIgnoreRelationshipPatternMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: (id: string) => api.relationshipPatterns.ignore({ projectId: projectId!, id }),
    onSuccess() {
      if (projectId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.projects.relationshipPatterns(projectId) })
      }
    }
  })
}

export const useDeleteRelationshipPatternMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: ({ id, deleteExisting }: { id: string; deleteExisting?: boolean }) =>
      api.relationshipPatterns.delete({ projectId: projectId!, id, deleteExisting }),
    onSuccess() {
      if (projectId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.projects.relationshipPatterns(projectId) })
        queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'records'] })
        queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'record-relations'] })
        queryClient.invalidateQueries({ queryKey: ['records'] })
      }
      toast({ title: 'Relationship pattern deleted' })
    }
  })
}
