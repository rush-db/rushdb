import { action } from 'nanostores'

import { api } from '~/lib/api'
import { createAsyncStore, createMutator } from '~/lib/fetcher'
import { $router, isProjectPage, redirectRoute } from '~/lib/router'

import type { Workspace } from '../types'

import { $currentWorkspaceId } from './current'
import { toast } from '~/elements/Toast.tsx'
import { $workspacesList } from '~/features/workspaces/stores/workspaces.ts'

export const $currentWorkspace = createAsyncStore({
  key: '$currentWorkspace',
  async fetcher(init) {
    const id = $currentWorkspaceId.get()

    if (!id) {
      return
    }

    return await api.workspaces.workspace({ id }, init)
  },
  deps: [$currentWorkspaceId]
})

export const setCurrentWorkspace = action(
  $currentWorkspaceId,
  'setCurrentWorkspace',
  (store, id: Workspace['id']) => {
    store.set(id)
    if (isProjectPage($router.get())) {
      redirectRoute('home')
    }
  }
)

export const leaveWorkspace = createMutator({
  async fetcher({ init, id }: { id: string; init?: RequestInit }) {
    return api.workspaces.leaveWorkspace({ id, init })
  },
  invalidates: [$workspacesList],
  onSuccess() {
    toast({ title: 'You left the workspace', duration: 3000 })
    setCurrentWorkspace(undefined as any)
    redirectRoute('home')
  },
  onError(err) {
    toast({
      title: 'Could not leave workspace',
      description: err instanceof Error ? err.message : ''
    })
  }
})
