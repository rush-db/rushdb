import { useState } from 'react'

import type { TDialogProps } from '~/elements/Dialog'

import { Button } from '~/elements/Button'
import { Close, Dialog, DialogFooter, DialogTitle } from '~/elements/Dialog'
import { TextField } from '~/elements/Input'

import { useCurrentWorkspaceQuery, useWorkspacesQuery } from '../hooks/useWorkspaceQueries'
import { useDeleteWorkspaceMutation } from '../hooks/useWorkspaceMutations'

const CONFIRM_WORD = 'delete'

export function DeleteWorkspaceDialog({ ...props }: TDialogProps) {
  const { data: workspaces } = useWorkspacesQuery()
  const { data: workspace } = useCurrentWorkspaceQuery()
  const { isPending: loading, mutateAsync: mutate } = useDeleteWorkspaceMutation()
  const [open, setOpen] = useState(false)
  const [value, setValue] = useState('')

  const singleWorkspace = workspaces && workspaces.length < 2

  const title =
    singleWorkspace ? 'You cannot delete your workspace' : `Delete "${workspace?.name ?? 'workspace'}"?`

  const confirmed = !singleWorkspace && value === CONFIRM_WORD

  const handleConfirm = async () => {
    if (!confirmed || !workspace?.id) return
    await mutate({ id: workspace.id })
    setOpen(false)
  }

  return (
    <Dialog
      className="justify-center gap-3"
      onOpenChange={(next) => {
        setOpen(next)
        if (!next) setValue('')
      }}
      open={open}
      {...props}
    >
      <DialogTitle className="text-base font-bold">{title}</DialogTitle>

      {singleWorkspace ?
        <p className="text-sm text-content2">
          Your account has only one workspace. Create a new one before deleting this one.
        </p>
      : <>
          <p className="text-sm text-content2">
            All projects and their data inside this workspace will be permanently destroyed. This action
            cannot be undone.
          </p>
          <p className="text-sm text-content2">
            Type <span className="font-medium text-danger">{CONFIRM_WORD}</span> to confirm:
          </p>
          <TextField
            autoFocus
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && confirmed) handleConfirm()
            }}
            placeholder={CONFIRM_WORD}
          />
        </>
      }

      <DialogFooter className="mt-2 flex-col sm:flex-row">
        {!singleWorkspace && (
          <Button
            className="sm:order-2 sm:flex-1"
            disabled={!confirmed}
            loading={loading}
            onClick={handleConfirm}
            variant="danger"
          >
            Delete workspace
          </Button>
        )}
        <Close asChild disabled={loading}>
          <Button className="sm:flex-1" variant="secondary">
            Cancel
          </Button>
        </Close>
      </DialogFooter>
    </Dialog>
  )
}
