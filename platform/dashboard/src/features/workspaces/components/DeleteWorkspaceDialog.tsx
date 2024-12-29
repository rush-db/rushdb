import { useStore } from '@nanostores/react'

import type { TDialogProps } from '~/elements/Dialog'

import { ConfirmDialog } from '~/elements/ConfirmDialog'

import { $currentWorkspace } from '../stores/current-workspace'
import { deleteWorkspace } from '../stores/mutations'
import { $workspacesList } from '../stores/workspaces'

export function DeleteWorkspaceDialog({ ...props }: TDialogProps) {
  const { data: workspaces } = useStore($workspacesList)
  const { data: workspace } = useStore($currentWorkspace)
  const { loading, mutate } = useStore(deleteWorkspace)

  const singleWorkspace = workspaces && workspaces?.length < 2

  const title = singleWorkspace
    ? `You cannot delete your workspace`
    : `Do you really want to delete "${workspace?.name ?? 'project'}"?`

  const description = singleWorkspace
    ? `Your account has only workspace available, create a new one to continue`
    : `All of your data related to the project will be lost`

  const handler = () => {
    return mutate({ id: workspace!.id })
  }

  return (
    <ConfirmDialog
      description={description}
      handler={singleWorkspace || !workspace?.id ? undefined : handler}
      loading={loading}
      title={title}
      {...props}
    />
  )
}
