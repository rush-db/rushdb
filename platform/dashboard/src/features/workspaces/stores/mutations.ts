import { useStore } from '@nanostores/react'

import type { ApiParams } from '~/lib/api'

import { toast } from '~/elements/Toast'
import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'
import { redirectRoute } from '~/lib/router'

import { $currentWorkspace, setCurrentWorkspace } from './current-workspace'
import { $workspacesList } from './workspaces'

export const updateWorkspace = createMutator<Parameters<typeof api.workspaces.update>[0]>({
  invalidates: [$currentWorkspace],
  async fetcher({ init, ...workspace }) {
    return await api.workspaces.update(workspace, init)
  }
})

export const deleteWorkspace = createMutator<Parameters<typeof api.workspaces.update>[0]>({
  invalidates: [$workspacesList, $currentWorkspace],
  async fetcher({ init, id }) {
    if (!id) {
      return
    }

    return await api.workspaces.delete({ id }, init)
  }
})

export const createWorkspace = createMutator({
  invalidates: [$workspacesList],
  async fetcher({ init, name }: ApiParams<typeof api.workspaces.create>) {
    return await api.workspaces.create({ init, name })
  },
  onSuccess(data) {
    setCurrentWorkspace(data.id)
    redirectRoute('home')
  }
})
