import { action } from 'nanostores'

import { api } from '~/lib/api'
import { createAsyncStore } from '~/lib/fetcher'
import { $router, isProjectPage, redirectRoute } from '~/lib/router'

import type { Workspace } from '../types'

import { $currentWorkspaceId } from './current'

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
