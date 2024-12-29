import { useState } from 'react'

import { Button } from '~/elements/Button'
import { GlobalNotification } from '~/elements/GlobalNotification'
import { resendConfirmationLink } from '~/features/auth/stores/auth'
import { useUser } from '~/features/auth/stores/user'

export function ConfirmEmailNotification() {
  const [didResend, setResent] = useState(false)
  const user = useUser()

  if (user.confirmed) {
    return null
  }

  return (
    <GlobalNotification
      action={
        <Button
          disabled={didResend}
          onClick={() => resendConfirmationLink().then(() => setResent(true))}
          size="small"
          variant="ghost"
        >
          {didResend ? 'Sent' : 'Resend'}
        </Button>
      }
      description={'Please follow a link in your inbox to confirm it'}
      title={'Your e-mail needs confirmation.'}
    />
  )
}
