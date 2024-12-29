import { $currentWorkspaceId } from '~/features/workspaces/stores/current'
import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'
import { $router, isProjectPage, redirectRoute } from '~/lib/router'

import type { Project } from '../types'

import { $workspaceProjects } from '../../workspaces/stores/projects'
import { $currentProject } from './current-project'
import { addToken } from '~/features/tokens/stores/tokens'

export const deleteProject = createMutator<Pick<Project, 'id'>>({
  async fetcher({ init, id }) {
    return await api.projects.delete({ init, id })
  },
  invalidates: [$workspaceProjects],
  onSuccess() {
    if (isProjectPage($router.get())) {
      redirectRoute('projects')
    }
  }
})

export const updateProject = createMutator<Partial<Project>>({
  invalidates: [$workspaceProjects, $currentProject],
  async fetcher({ init, ...body }) {
    return await api.projects.update({ init, ...body })
  }
})

export const createProject = createMutator<Partial<Project>, Project>({
  invalidates: [$workspaceProjects],
  async fetcher({ init, ...body }) {
    const workspaceId = $currentWorkspaceId.get()

    if (!workspaceId) {
      return
    }

    const project = await api.projects.create({ init, ...body })

    try {
      await addToken.get().mutate({
        projectId: project.id,
        name: 'Initial',
        description: 'Initial API Key to get you started quickly.',
        noExpire: true
      })
    } catch (error) {
      console.error(error)
    }

    return project
  },
  onSuccess(project) {
    redirectRoute('projectHelp', { id: project.id })
  }
})
