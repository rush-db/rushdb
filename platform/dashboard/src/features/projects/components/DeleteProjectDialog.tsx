import { useStore } from '@nanostores/react'

import type { TDialogProps } from '~/elements/Dialog'

import { ConfirmDialog } from '~/elements/ConfirmDialog'

import type { Project } from '../types'

import { $currentProject } from '../stores/current-project'
import { deleteProject } from '../stores/project'

export function DeleteProjectDialog({
  projectId,
  ...props
}: TDialogProps & {
  projectId: Project['id']
}) {
  const { data: project } = useStore($currentProject)
  const { loading, mutate } = useStore(deleteProject)

  return (
    <ConfirmDialog
      description={`All of your data related to the project will be lost`}
      handler={() => mutate({ id: projectId })}
      loading={loading}
      title={`Do you really want to delete "${project?.name ?? 'project'}"?`}
      {...props}
    />
  )
}
