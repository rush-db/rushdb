import { useStore } from '@nanostores/react'
import { useEffect } from 'react'

import { oauthLogin } from '~/features/auth/stores/auth'
import { $searchParams } from '~/lib/router'
import { DialogLoadingOverlay } from '~/elements/Dialog.tsx'

export function OauthPage() {
  const searchParams = useStore($searchParams)

  useEffect(() => {
    oauthLogin(searchParams.token)
  }, [searchParams])

  return <DialogLoadingOverlay />
}
