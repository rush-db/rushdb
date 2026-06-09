import { action } from 'nanostores'

import { $router, isProjectPage, redirectRoute } from '~/lib/router'

import type { Workspace } from '../types'

import { $currentWorkspaceId } from './current'

export const setCurrentWorkspace = action(
  $currentWorkspaceId,
  'setCurrentWorkspace',
  (store, id: Workspace['id'] | undefined) => {
    store.set(id)

    if (isProjectPage($router.get())) {
      redirectRoute('home')
    }
  }
)
