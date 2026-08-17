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

/** Provider catalog partitioned by the workspace plan (synx owns the union). */
export const useConnectorCatalogQuery = () => {
  const { data: platformSettings } = usePlatformSettings()
  return useQuery({
    queryKey: ['connectors', 'catalog'],
    queryFn: () => api.connectors.catalog(),
    enabled: platformSettings?.synxEnabled === true,
    staleTime: 60_000
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

export const useConnectorRunsQuery = (connectorId?: string) => {
  const projectId = useStore($currentProjectId)
  const { data: platformSettings } = usePlatformSettings()
  return useQuery({
    queryKey:
      projectId && connectorId ?
        [...queryKeys.projects.connectors(projectId), connectorId, 'runs']
      : ['projects', 'connectors', connectorId, 'runs'],
    queryFn: () => api.connectors.runs({ projectId: projectId!, id: connectorId! }),
    enabled: !!projectId && !!connectorId && platformSettings?.synxEnabled === true,
    refetchInterval: 5000
  })
}

export const useConnectorRejectionsQuery = (connectorId?: string) => {
  const projectId = useStore($currentProjectId)
  const { data: platformSettings } = usePlatformSettings()
  return useQuery({
    queryKey:
      projectId && connectorId ?
        [...queryKeys.projects.connectors(projectId), connectorId, 'rejections']
      : ['projects', 'connectors', connectorId, 'rejections'],
    queryFn: () => api.connectors.rejections({ projectId: projectId!, id: connectorId! }),
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

export const useUpdateConnectorMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: ({ id, ...body }: { id: string } & Partial<CreateConnectorInput>) =>
      api.connectors.update({ projectId: projectId!, id, ...body }),
    onSuccess() {
      if (projectId) queryClient.invalidateQueries({ queryKey: queryKeys.projects.connectors(projectId) })
    }
  })
}

export const useDeleteConnectorMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: ({ id, deleteRecords = false }: { id: string; deleteRecords?: boolean }) =>
      api.connectors.remove({ projectId: projectId!, id, deleteRecords }),
    onSuccess() {
      if (projectId) queryClient.invalidateQueries({ queryKey: queryKeys.projects.connectors(projectId) })
    }
  })
}

export type ConnectorAction = 'pause' | 'resume' | 'resnapshot' | 'start' | 'replay' | 'cancel' | 'test'

export const useConnectorActionMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: ({ id, action }: { id: string; action: ConnectorAction }) =>
      api.connectors.action({ projectId: projectId!, id, action }),
    onSuccess() {
      if (projectId) queryClient.invalidateQueries({ queryKey: queryKeys.projects.connectors(projectId) })
    }
  })
}

export type DiscoveredStream = {
  namespace?: string
  name: string
  targetLabel?: string
  estimatedRecords?: number
}

const POLL_INTERVAL_MS = 1500
const POLL_TIMEOUT_MS = 30_000

/**
 * Poll a connector command until it completes (or fails), returning its raw
 * result object. Throws with the worker's error message on failure.
 */
const pollCommand = async ({
  projectId,
  connectorId,
  commandId
}: {
  projectId: string
  connectorId: string
  commandId: string
}): Promise<Record<string, unknown>> => {
  const deadline = Date.now() + POLL_TIMEOUT_MS
  while (Date.now() < deadline) {
    const commands = await api.connectors.commands({ projectId, id: connectorId })
    const command = commands.find((c) => c.id === commandId)
    if (!command) {
      await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS))
      continue
    }
    if (command.status === 'completed') {
      try {
        return JSON.parse(command.result ?? '{}') as Record<string, unknown>
      } catch {
        return {}
      }
    }
    if (command.status === 'failed') {
      throw new Error(command.errorMessage ?? 'Command failed')
    }
    await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS))
  }
  throw new Error('Command timed out')
}

/**
 * Enqueue a discover command for a (just-created) connector and poll until the
 * worker reports `completed`, returning the discovered streams.
 */
export const discoverConnectorStreams = async ({
  projectId,
  connectorId
}: {
  projectId: string
  connectorId: string
}): Promise<DiscoveredStream[]> => {
  const queued = await api.connectors.discover({ projectId, id: connectorId })
  const result = await pollCommand({ projectId, connectorId, commandId: queued.commandId })
  return Array.isArray(result.streams) ? (result.streams as DiscoveredStream[]) : []
}

/**
 * Enqueue a databases command and return the available databases for a
 * connector that has not yet picked a database.
 */
export const listConnectorDatabases = async ({
  projectId,
  connectorId
}: {
  projectId: string
  connectorId: string
}): Promise<string[]> => {
  const queued = await api.connectors.databases({ projectId, id: connectorId })
  const result = await pollCommand({ projectId, connectorId, commandId: queued.commandId })
  return Array.isArray(result.databases) ? (result.databases as string[]) : []
}
