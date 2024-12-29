import { useStore } from '@nanostores/react'
import { useEffect, useState } from 'react'

import { confirmEmail } from '~/features/auth/stores/auth'
import { $searchParams, getRoutePath, redirectRoute } from '~/lib/router'
import { $user } from '~/features/auth/stores/user.ts'
import { ConfirmEmailErrorCodes } from '~/features/auth/constants.ts'
import { Dialog, DialogFooter, DialogTitle } from '~/elements/Dialog.tsx'
import { AlertTriangle } from 'lucide-react'
import { Button } from '~/elements/Button.tsx'

export function ConfirmEmail() {
  const [isOpen, setOpen] = useState(false)
  const [isError, setIsError] = useState(false)

  const searchParams = useStore($searchParams)
  const user = useStore($user)
  const route = user.isLoggedIn ? 'home' : 'signin'

  const { token } = searchParams

  if (!token) {
    redirectRoute(route)
    return null
  }

  useEffect(() => {
    const confirmEmailData = async () => {
      return await confirmEmail(searchParams)
    }

    confirmEmailData()
      .catch((error) => {
        if (
          error.response.status === ConfirmEmailErrorCodes.EmailTokenExpired
        ) {
          setIsError(true)
        }
      })
      .finally(() => {
        setOpen(true)
      })
  }, [])

  function handleCloseConfirmationModal() {
    setOpen(false)
    redirectRoute(route)
  }

  return (
    <>
      <Dialog
        className="gap-5"
        onOpenChange={handleCloseConfirmationModal}
        open={isOpen}
      >
        <DialogTitle>
          {isError ? (
            <>
              <AlertTriangle /> Email Confirmation Failed
            </>
          ) : (
            'Email Confirmed Successfully'
          )}
        </DialogTitle>

        <p className="min-h-sm">
          {isError
            ? 'Your email is already confirmed, or the confirmation link has expired.'
            : 'Your email has been successfully confirmed. You can now proceed.'}
        </p>

        <DialogFooter>
          <Button
            as="a"
            href={getRoutePath(route)}
            onClick={handleCloseConfirmationModal}
            variant="accent"
          >
            {isError ? 'Got it' : 'Continue'}
          </Button>
        </DialogFooter>
      </Dialog>
    </>
  )
}
