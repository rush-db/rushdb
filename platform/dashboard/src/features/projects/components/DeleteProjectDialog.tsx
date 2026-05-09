import type { TDialogProps } from '~/elements/Dialog'

import { ConfirmDialog } from '~/elements/ConfirmDialog'

import type { Project } from '../types'

import { useCurrentProjectQuery } from '../hooks/useProjectQueries'
import { useDeleteProjectMutation } from '../hooks/useProjectMutations'

export function DeleteProjectDialog({
  projectId,
  ...props
}: TDialogProps & {
  projectId: Project['id']
}) {
  const { data: project } = useCurrentProjectQuery()
  const { isPending: loading, mutateAsync } = useDeleteProjectMutation()

  return (
    <ConfirmDialog
      description={`All of your data related to the project will be lost`}
      handler={() => mutateAsync({ id: projectId })}
      loading={loading}
      title={`Do you really want to delete "${project?.name ?? 'project'}"`}
      {...props}
    />
  )
}
