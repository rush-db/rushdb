import { useStore } from '@nanostores/react'
import { AlertTriangle } from 'lucide-react'
import { atom } from 'nanostores'

import { Button } from '~/elements/Button'
import { Dialog, DialogFooter, DialogTitle } from '~/elements/Dialog'
import { getRoutePath } from '~/lib/router'

export const $limitReachModalOpen = atom<boolean>(false)

// TODO
export function LimitReachedModal() {
  const open = useStore($limitReachModalOpen)

  return (
    <Dialog
      className="gap-5"
      onOpenChange={$limitReachModalOpen.set}
      open={open}
    >
      <DialogTitle>
        <AlertTriangle /> Limit Reached
      </DialogTitle>

      <p className="min-h-sm">
        You've reached the limit of your current plan. To continue enjoying all
        the features, please consider upgrading to a subscription plan.
      </p>

      <DialogFooter>
        <Button
          as="a"
          href={getRoutePath('workspaceBilling')}
          onClick={() => $limitReachModalOpen.set(false)}
          variant="accent"
        >
          Check subscriptions
        </Button>
      </DialogFooter>
    </Dialog>
  )
}
