import type { ChangeEvent } from 'react'

import { action, atom, computed } from 'nanostores'

import { $currentWorkspaceId } from '~/features/workspaces/stores/current'
import { api } from '~/lib/api'
import { createAsyncStore } from '~/lib/fetcher'
import { normalizeString } from '~/lib/utils'
import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace.ts'
import { $platformSettings } from '~/features/auth/stores/settings.ts'

export const $workspaceProjects = createAsyncStore({
  key: '$workspaceProjects',
  async fetcher(init) {
    const workspaceId = $currentWorkspaceId.get()

    if (!workspaceId) {
      return
    }

    return await api.projects.list(init)
  },
  deps: [$currentWorkspaceId]
})

export const $projectsQuery = atom<string>('')

export const $filteredProjects = computed([$workspaceProjects, $projectsQuery], (allProjects, q) =>
  allProjects?.data?.filter((project) => normalizeString(project.name).includes(normalizeString(q)))
)

export const $showUpgrade = computed(
  [$workspaceProjects, $currentWorkspace, $platformSettings],
  (projects, workspace) => {
    const maxProjects = workspace.data?.limits?.projects ?? Infinity

    return (
        !$platformSettings.get().data?.selfHosted &&
          typeof maxProjects !== 'undefined' &&
          typeof projects.data !== 'undefined'
      ) ?
        projects.data?.length >= maxProjects
      : false
  }
)

export const filterProjects = action(
  $projectsQuery,
  'filterProjects',
  (store, event: ChangeEvent<HTMLInputElement>) => {
    store.set(event.target.value)
  }
)
