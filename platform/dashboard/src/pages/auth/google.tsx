import { useStore } from '@nanostores/react'
import { useEffect } from 'react'

import { logInGoogle } from '~/features/auth/stores/auth'
import { $searchParams } from '~/lib/router'
import { DialogLoadingOverlay } from '~/elements/Dialog.tsx'

export function AuthGoogle() {
  const searchParams = useStore($searchParams)

  useEffect(() => {
    logInGoogle(searchParams)
  }, [searchParams])

  return <DialogLoadingOverlay />
}
