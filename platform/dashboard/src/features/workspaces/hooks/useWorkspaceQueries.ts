import { useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import { $token } from '~/features/auth/stores/token'
import { $currentWorkspaceId } from '~/features/workspaces/stores/current'
import { setCurrentWorkspace } from '~/features/workspaces/stores/current-workspace'
import {
  workspacesQueryOptions,
  workspaceDetailQueryOptions,
  workspaceProjectsQueryOptions,
  workspaceUsersQueryOptions,
  workspaceAccessListQueryOptions,
  workspacePendingInvitesQueryOptions
} from '../queries/workspaceQueries'

export const useWorkspacesQuery = () => {
  const token = useStore($token)
  const currentWorkspaceId = useStore($currentWorkspaceId)
  const query = useQuery(workspacesQueryOptions(token))

  useEffect(() => {
    const workspaces = query.data

    if (!workspaces?.length) {
      return
    }

    const hasCurrentWorkspace =
      currentWorkspaceId ? workspaces.some((workspace) => workspace.id === currentWorkspaceId) : false

    if (!hasCurrentWorkspace) {
      setCurrentWorkspace(workspaces[0].id)
    }
  }, [currentWorkspaceId, query.data])

  return query
}

export const useCurrentWorkspaceQuery = () => {
  const workspaceId = useStore($currentWorkspaceId)
  return useQuery(workspaceDetailQueryOptions(workspaceId))
}

export const useWorkspaceProjectsQuery = () => {
  const workspaceId = useStore($currentWorkspaceId)
  return useQuery(workspaceProjectsQueryOptions(workspaceId))
}

export const useWorkspaceUsersQuery = () => {
  const workspaceId = useStore($currentWorkspaceId)
  return useQuery(workspaceUsersQueryOptions(workspaceId))
}

export const useWorkspaceAccessListQuery = () => {
  const workspaceId = useStore($currentWorkspaceId)
  return useQuery(workspaceAccessListQueryOptions(workspaceId))
}

export const useWorkspacePendingInvitesQuery = () => {
  const workspaceId = useStore($currentWorkspaceId)
  return useQuery(workspacePendingInvitesQueryOptions(workspaceId))
}
