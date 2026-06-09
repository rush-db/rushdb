import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import type { Workspace } from '~/features/workspaces/types'
import type { InviteToWorkspaceDto, RevokeAccessDto, WorkspaceAccessList } from '~/features/workspaces/types'

import { api } from '~/lib/api'
import { toast } from '~/elements/Toast'
import { queryKeys } from '~/lib/queryKeys'
import { $currentWorkspaceId } from '~/features/workspaces/stores/current'
import { setCurrentWorkspace } from '~/features/workspaces/stores/current-workspace'
import { redirectRoute } from '~/lib/router'

export const useCreateWorkspaceMutation = () => {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ name }: Pick<Workspace, 'name'>) => api.workspaces.create({ name }),
    onSuccess(data) {
      queryClient.setQueryData<Workspace[]>(queryKeys.workspaces.all(), (workspaces = []) => [
        data,
        ...workspaces.filter((workspace) => workspace.id !== data.id)
      ])
      queryClient.setQueryData(queryKeys.workspaces.detail(data.id), data)
      setCurrentWorkspace(data.id)
      redirectRoute('home')
      queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.all() })
    }
  })
}

export const useUpdateWorkspaceMutation = () => {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (workspace: Partial<Workspace> & Pick<Workspace, 'id'>) =>
      api.workspaces.update(workspace, {} as RequestInit),
    onSuccess(_, vars) {
      queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.detail(vars.id) })
      queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.all() })
    }
  })
}

export const useDeleteWorkspaceMutation = () => {
  const queryClient = useQueryClient()
  const currentWorkspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: ({ id }: Pick<Workspace, 'id'>) => api.workspaces.delete({ id }, {} as RequestInit),
    onSuccess() {
      queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.all() })
      const cached = queryClient.getQueryData<Workspace[]>(queryKeys.workspaces.all()) ?? []
      const next = cached.find((w) => w.id !== currentWorkspaceId)
      if (next?.id) {
        setCurrentWorkspace(next.id)
        redirectRoute('home')
      } else {
        redirectRoute('newWorkspace')
      }
    }
  })
}

export const useLeaveWorkspaceMutation = () => {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id }: { id: string }) => api.workspaces.leaveWorkspace({ id }),
    onSuccess() {
      queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.all() })
      toast({ title: 'You left the workspace', duration: 3000 })
      setCurrentWorkspace(undefined)
      redirectRoute('home')
    },
    onError(err) {
      toast({
        title: 'Could not leave workspace',
        description: err instanceof Error ? err.message : ''
      })
    }
  })
}

export const useInviteUserMutation = () => {
  const queryClient = useQueryClient()
  const workspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: ({ email, projectIds }: InviteToWorkspaceDto) => {
      if (!workspaceId) throw new Error('No workspace selected')
      return api.workspaces.inviteUser({ id: workspaceId, email, projectIds })
    },
    onSuccess() {
      if (!workspaceId) return
      queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.users(workspaceId) })
      queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.pendingInvites(workspaceId) })
    }
  })
}

export const useRevokeAccessMutation = () => {
  const queryClient = useQueryClient()
  const workspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: ({ userIds }: Pick<RevokeAccessDto, 'userIds'>) => {
      if (!workspaceId) throw new Error('No workspace selected')
      return api.workspaces.revokeAccess({ id: workspaceId, userIds })
    },
    onSuccess() {
      if (!workspaceId) return
      queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.users(workspaceId) })
      queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.accessList(workspaceId) })
    }
  })
}

export const useUpdateAccessListMutation = () => {
  const queryClient = useQueryClient()
  const workspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: ({ accessMap }: { accessMap: WorkspaceAccessList }) => {
      if (!workspaceId) throw new Error('No workspace selected')
      return api.workspaces.updateAccessList({ id: workspaceId, accessMap })
    },
    onSuccess() {
      if (!workspaceId) return
      queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.accessList(workspaceId) })
    }
  })
}

export const useRemovePendingInviteMutation = () => {
  const queryClient = useQueryClient()
  const workspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: ({ email }: { email: string }) => {
      if (!workspaceId) throw new Error('No workspace selected')
      return api.workspaces.removePendingInvite({ id: workspaceId, email })
    },
    onSuccess() {
      if (!workspaceId) return
      queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.pendingInvites(workspaceId) })
    }
  })
}

export const useAcceptInvitationMutation = () => {
  return useMutation({
    mutationFn: ({ token }: { token: string }) => api.workspaces.acceptInvitation({ token }),
    onSuccess({ workspaceId }) {
      if (workspaceId) {
        setCurrentWorkspace(workspaceId)
        toast({
          title: 'Invitation accepted',
          description: "You've been added to the workspace",
          duration: 5000
        })
        redirectRoute('home')
      }
    },
    onError(error) {
      toast({
        title: 'Error accepting invitation',
        description: error instanceof Error ? error.message : 'Failed to accept workspace invitation',
        variant: 'danger',
        duration: 5000
      })
    }
  })
}
