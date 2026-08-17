import { useStore } from '@nanostores/react'
import { useEffect, useRef } from 'react'

import { logInGitHub } from '~/features/auth/stores/auth'
import { $searchParams, redirectRoute } from '~/lib/router'
import { DialogLoadingOverlay } from '~/elements/Dialog.tsx'

export function AuthGitHub() {
  const searchParams = useStore($searchParams)
  const hasAttemptedExchange = useRef(false)

  useEffect(() => {
    if (hasAttemptedExchange.current || (!searchParams.code && !searchParams.error)) {
      return
    }

    hasAttemptedExchange.current = true

    logInGitHub(searchParams).catch(() => {
      redirectRoute('signin')
    })
  }, [searchParams])

  return <DialogLoadingOverlay />
}
