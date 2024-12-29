import { $token } from '~/features/auth/stores/token'
import { api } from '~/lib/api'
import { createAsyncStore } from '~/lib/fetcher'

import { $currentWorkspaceId } from './current'

export const $workspacesList = createAsyncStore({
  key: '$workspacesList',
  async fetcher(init) {
    return await api.workspaces.list(init)
  },
  mustHaveDeps: [$token]
})

$workspacesList.subscribe((newValue) => {
  const current = $currentWorkspaceId.get()

  if (!current && newValue && newValue?.data && newValue.data.length > 0) {
    if (typeof newValue.data[0].id === 'string') {
      $currentWorkspaceId.set(newValue.data[0].id)
    }
  }
})
