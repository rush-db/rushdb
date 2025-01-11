import type { ReactNode } from 'react'

import { useRef, useState } from 'react'

import alert from '~/assets/Alert.png'

import { DialogTitle, TDialogProps } from './Dialog'

import { Button } from './Button'
import { Close, Dialog, DialogFooter } from './Dialog'
import { DialogContent } from '@radix-ui/react-dialog'

export function ConfirmDialog({
  description = 'This action cannot be undone',
  handler,
  loading,
  title,
  trigger
}: TDialogProps & {
  description?: ReactNode
  handler?: () => Promise<unknown> | undefined
  loading?: boolean
  title?: ReactNode
}) {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLButtonElement>(null)

  return (
    <Dialog
      onOpenAutoFocus={(event) => {
        event.preventDefault()
        ref.current?.focus()
      }}
      className="justify-center gap-3"
      onOpenChange={setOpen}
      open={open}
      trigger={trigger}
    >
      <img alt="alert" className="mx-auto h-[100px] w-[100px] sm:h-[160px] sm:w-[160px]" src={alert} />
      <DialogTitle className="text-base font-bold">{title}</DialogTitle>

      {description && (
        <DialogContent>
          <p className="text-content2">{description}</p>
        </DialogContent>
      )}
      <DialogFooter className="mt-5 flex-col sm:flex-row">
        {handler && (
          <Button
            className="sm:order-2 sm:flex-1"
            loading={loading}
            onClick={() => handler()?.then(() => setOpen(false))}
            variant="danger"
          >
            Confirm
          </Button>
        )}
        <Close asChild disabled={loading} ref={ref}>
          <Button className="sm:flex-1" variant="secondary">
            Cancel
          </Button>
        </Close>
      </DialogFooter>
    </Dialog>
  )
}
