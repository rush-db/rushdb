import { Trash2 } from 'lucide-react'
import { useState } from 'react'

import { Button } from '~/elements/Button'
import { Close, Dialog, DialogFooter, DialogTitle } from '~/elements/Dialog'
import { cn } from '~/lib/utils'

/**
 * Delete-confirmation for a data source. Always asks for explicit confirmation;
 * also offers a (deselected by default) checkbox to delete the synced records
 * in the project too, not just the connector.
 */
export function DeleteConnectorDialog({
  connectorName,
  onDelete,
  loading
}: {
  connectorName: string
  onDelete: (deleteRecords: boolean) => void
  loading?: boolean
}) {
  const [open, setOpen] = useState(false)
  const [deleteRecords, setDeleteRecords] = useState(false)

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!loading) {
          setOpen(next)
        }
      }}
      trigger={
        <Button size="xsmall" variant="dangerGhost">
          <Trash2 size={14} />
          Delete
        </Button>
      }
    >
      <DialogTitle className="text-base font-bold">Delete data source</DialogTitle>

      <p className="mt-2 text-sm text-content2">
        Delete “{connectorName}”? The connector configuration, credentials, and sync offsets will be removed.
        Synced records in this project will be kept.
      </p>

      <label
        className={cn(
          'mt-4 flex cursor-pointer items-start gap-3 rounded-md border p-3 text-sm',
          deleteRecords ? 'border-danger/50 bg-danger/5' : 'hover:bg-secondary/40'
        )}
      >
        <input
          type="checkbox"
          checked={deleteRecords}
          onChange={(event) => setDeleteRecords(event.target.checked)}
          className="mt-0.5 size-4 accent-danger"
        />
        <span>
          <span className="font-medium text-danger">Also delete synced records</span>
          <span className="block text-content2">
            Permanently remove every record this source synced into the project. This cannot be undone.
          </span>
        </span>
      </label>

      <DialogFooter className="mt-5">
        <Button
          className="sm:order-2 sm:flex-1"
          loading={loading}
          onClick={() => onDelete(deleteRecords)}
          variant="danger"
        >
          Delete {deleteRecords ? 'source and records' : 'source'}
        </Button>
        <Close asChild>
          <Button className="sm:flex-1" disabled={loading} variant="secondary">
            Cancel
          </Button>
        </Close>
      </DialogFooter>
    </Dialog>
  )
}
