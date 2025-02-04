import { useStore } from '@nanostores/react'
import { useEffect } from 'react'

import { logInGitHub } from '~/features/auth/stores/auth'
import { $searchParams } from '~/lib/router'
import { DialogLoadingOverlay } from '~/elements/Dialog.tsx'

export function AuthGitHub() {
  const searchParams = useStore($searchParams)

  useEffect(() => {
    logInGitHub(searchParams)
  }, [searchParams])

  return <DialogLoadingOverlay />
}
