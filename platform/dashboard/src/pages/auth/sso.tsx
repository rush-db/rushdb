import { useStore } from '@nanostores/react'
import { useEffect } from 'react'

import { logInSso } from '~/features/auth/stores/auth'
import { $searchParams, redirectRoute } from '~/lib/router'
import { DialogLoadingOverlay } from '~/elements/Dialog.tsx'
import { setCurrentWorkspace } from '~/features/workspaces/stores/current-workspace.ts'

export function AuthSso() {
  const searchParams = useStore($searchParams)

  useEffect(() => {
    const token = searchParams.token
    const workspaceId = searchParams.workspaceId
    const redirectUrl = searchParams.redirectUrl

    if (!token) {
      redirectRoute('signin')
      return
    }

    logInSso({ token, workspaceId })
      .then((possibleWorkspaceId) => {
        if (possibleWorkspaceId) {
          setCurrentWorkspace(possibleWorkspaceId)
        }
        if (redirectUrl && /^\//.test(redirectUrl)) {
          window.location.href = redirectUrl
        }
      })
      .catch(() => {
        redirectRoute('signin')
      })
  }, [searchParams])

  return <DialogLoadingOverlay />
}
