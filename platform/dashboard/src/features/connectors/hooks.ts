import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import { $currentProjectId } from '~/features/projects/stores/id'
import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'
import type { CreateConnectorInput } from '~/features/connectors/types'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'

export const useProjectConnectorsQuery = () => {
  const projectId = useStore($currentProjectId)
  const { data: platformSettings } = usePlatformSettings()
  return useQuery({
    queryKey: projectId ? queryKeys.projects.connectors(projectId) : ['projects', 'connectors'],
    queryFn: () => api.connectors.list({ projectId: projectId! }),
    enabled: !!projectId && platformSettings?.synxEnabled === true,
    refetchInterval: 5000
  })
}

export const useProjectConnectorQuery = (connectorId?: string) => {
  const projectId = useStore($currentProjectId)
  const { data: platformSettings } = usePlatformSettings()
  return useQuery({
    queryKey:
      projectId && connectorId ?
        [...queryKeys.projects.connectors(projectId), connectorId]
      : ['projects', 'connectors', connectorId],
    queryFn: () => api.connectors.get({ projectId: projectId!, id: connectorId! }),
    enabled: !!projectId && !!connectorId && platformSettings?.synxEnabled === true,
    refetchInterval: 5000
  })
}

export const useConnectorEventsQuery = (connectorId?: string) => {
  const projectId = useStore($currentProjectId)
  const { data: platformSettings } = usePlatformSettings()
  return useQuery({
    queryKey:
      projectId && connectorId ?
        [...queryKeys.projects.connectors(projectId), connectorId, 'events']
      : ['projects', 'connectors', connectorId, 'events'],
    queryFn: () => api.connectors.events({ projectId: projectId!, id: connectorId! }),
    enabled: !!projectId && !!connectorId && platformSettings?.synxEnabled === true,
    refetchInterval: 5000
  })
}

export const useCreateConnectorMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: (body: CreateConnectorInput) => api.connectors.create({ ...body, projectId: projectId! }),
    onSuccess() {
      if (projectId) queryClient.invalidateQueries({ queryKey: queryKeys.projects.connectors(projectId) })
    }
  })
}

export const useConnectorActionMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: ({ id, action }: { id: string; action: 'pause' | 'resume' | 'resnapshot' | 'test' }) =>
      api.connectors.action({ projectId: projectId!, id, action }),
    onSuccess() {
      if (projectId) queryClient.invalidateQueries({ queryKey: queryKeys.projects.connectors(projectId) })
    }
  })
}
