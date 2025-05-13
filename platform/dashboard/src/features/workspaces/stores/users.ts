import { api } from '~/lib/api'
import { createAsyncStore, createMutator } from '~/lib/fetcher'
import type { ApiParams } from '~/lib/api'

import { $currentWorkspaceId } from './current'

// Store for the list of workspace users
export const $workspaceUsers = createAsyncStore({
  key: '$workspaceUsers',
  async fetcher(init) {
    const id = $currentWorkspaceId.get()

    if (!id) {
      return
    }

    return await api.workspaces.getUserList({ id, init })
  },
  deps: [$currentWorkspaceId]
})

// Store for the access list by projects
export const $workspaceAccessList = createAsyncStore({
  key: '$workspaceAccessList',
  async fetcher(init) {
    const id = $currentWorkspaceId.get()

    if (!id) {
      return
    }

    return await api.workspaces.getAccessList({ id, init })
  },
  deps: [$currentWorkspaceId]
})

// Mutator for inviting a user to the workspace
export const inviteUser = createMutator({
  async fetcher({ init, email, projectIds }: ApiParams<typeof api.workspaces.inviteUser>) {
    const id = $currentWorkspaceId.get()
    if (!id) {
      throw new Error('No workspace selected')
    }

    return await api.workspaces.inviteUser({ id, email, projectIds, init })
  },
  invalidates: [$workspaceUsers]
})

// Mutator for revoking user access
export const revokeAccess = createMutator({
  async fetcher({ init, userIds }: ApiParams<typeof api.workspaces.revokeAccess>) {
    const id = $currentWorkspaceId.get()
    if (!id) {
      throw new Error('No workspace selected')
    }

    return await api.workspaces.revokeAccess({ id, userIds, init })
  },
  invalidates: [$workspaceUsers, $workspaceAccessList]
})

// Mutator for updating the project access list
export const updateAccessList = createMutator({
  async fetcher({ init, accessMap }: ApiParams<typeof api.workspaces.updateAccessList>) {
    const id = $currentWorkspaceId.get()
    if (!id) {
      throw new Error('No workspace selected')
    }

    return await api.workspaces.updateAccessList({ id, accessMap, init })
  },
  invalidates: [$workspaceAccessList]
})
