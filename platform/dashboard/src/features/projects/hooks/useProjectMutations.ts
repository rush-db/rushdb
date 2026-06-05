import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import type { Project } from '~/features/projects/types'
import type { ProjectToken } from '~/features/tokens/types'

import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'
import { trackProjectCreated, trackApiKeyGenerated } from '~/lib/analytics'
import { $currentWorkspaceId } from '~/features/workspaces/stores/current'
import { $currentProjectId } from '~/features/projects/stores/id'
import { $router, isProjectPage, redirectRoute } from '~/lib/router'

export const useCreateProjectMutation = () => {
  const queryClient = useQueryClient()
  const workspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: async (body: Partial<Project>) => {
      if (!workspaceId) throw new Error('No workspace selected')
      const project = await api.projects.create({ ...body })
      // Create initial token
      try {
        await api.tokens.create({
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
      const projects = queryClient.getQueryData<{ id: string }[]>(
        workspaceId ? queryKeys.workspaces.projects(workspaceId) : []
      )
      trackProjectCreated({ isFirstProject: (projects?.length ?? 0) === 0 })
      trackApiKeyGenerated({ isInitialKey: true })
      if (workspaceId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.projects(workspaceId) })
      }
      const projectIsInactive = project.status === 'pending' || project.status === 'provisioning'
      if (!projectIsInactive) {
        redirectRoute('projectHelp', { id: project.id })
      } else {
        redirectRoute('projectSettings', { id: project.id })
      }
    }
  })
}

export const useUpdateProjectMutation = () => {
  const queryClient = useQueryClient()
  const workspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: (body: Partial<Project> & { id: string }) => api.projects.update(body),
    onSuccess(_, vars) {
      queryClient.invalidateQueries({ queryKey: queryKeys.projects.detail(vars.id!) })
      if (workspaceId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.projects(workspaceId) })
      }
    }
  })
}

export const useDeleteProjectMutation = () => {
  const queryClient = useQueryClient()
  const workspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: ({ id }: { id: string }) => api.projects.delete({ id }),
    onMutate: async ({ id }: { id: string }) => {
      if (!workspaceId) return { previousProjects: undefined as Project[] | undefined }

      const queryKey = queryKeys.workspaces.projects(workspaceId)
      await queryClient.cancelQueries({ queryKey })

      const previousProjects = queryClient.getQueryData<Project[]>(queryKey)
      queryClient.setQueryData<Project[]>(queryKey, (current = []) =>
        current.filter((project) => project.id !== id)
      )

      return { previousProjects }
    },
    onError(_error, _variables, context) {
      if (workspaceId && context?.previousProjects) {
        queryClient.setQueryData(queryKeys.workspaces.projects(workspaceId), context.previousProjects)
      }
    },
    onSuccess() {
      if (workspaceId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.projects(workspaceId) })
      }
      if (isProjectPage($router.get())) {
        redirectRoute('projects')
      }
    },
    onSettled() {
      if (workspaceId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.projects(workspaceId) })
      }
    }
  })
}

export const useAddTokenMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: (args: Parameters<typeof api.tokens.create>[0]) => api.tokens.create(args),
    onSuccess() {
      if (projectId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.projects.tokens(projectId) })
      }
    }
  })
}

export const useDeleteTokenMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  return useMutation({
    mutationFn: ({ tokenId }: { tokenId: ProjectToken['id'] }) => api.tokens.delete({ id: tokenId }),
    onSuccess() {
      if (projectId) {
        queryClient.invalidateQueries({ queryKey: queryKeys.projects.tokens(projectId) })
      }
    }
  })
}
