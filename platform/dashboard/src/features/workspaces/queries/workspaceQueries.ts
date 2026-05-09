import { queryOptions } from '@tanstack/react-query'

import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'

export const workspacesQueryOptions = (token: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.workspaces.all(),
    queryFn: () => api.workspaces.list({} as RequestInit),
    enabled: !!token
  })

export const workspaceDetailQueryOptions = (workspaceId: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.workspaces.detail(workspaceId!),
    queryFn: () => api.workspaces.workspace({ id: workspaceId! }, {} as RequestInit),
    enabled: !!workspaceId
  })

export const workspaceProjectsQueryOptions = (workspaceId: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.workspaces.projects(workspaceId!),
    queryFn: async () => {
      const result = await api.projects.list({} as RequestInit)
      return result.data
    },
    enabled: !!workspaceId
  })

export const workspaceUsersQueryOptions = (workspaceId: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.workspaces.users(workspaceId!),
    queryFn: () => api.workspaces.getUserList({ id: workspaceId! }),
    enabled: !!workspaceId
  })

export const workspaceAccessListQueryOptions = (workspaceId: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.workspaces.accessList(workspaceId!),
    queryFn: () => api.workspaces.getAccessList({ id: workspaceId! }),
    enabled: !!workspaceId
  })

export const workspacePendingInvitesQueryOptions = (workspaceId: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.workspaces.pendingInvites(workspaceId!),
    queryFn: () => api.workspaces.getPendingInvites({ id: workspaceId! }),
    enabled: !!workspaceId
  })
