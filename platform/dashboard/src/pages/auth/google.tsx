import { useStore } from '@nanostores/react'
import { useEffect } from 'react'

import { logInGoogle } from '~/features/auth/stores/auth'
import { $searchParams } from '~/lib/router'
import { DialogLoadingOverlay } from '~/elements/Dialog.tsx'
import { setCurrentWorkspace } from '~/features/workspaces/stores/current-workspace.ts'

export function AuthGoogle() {
  const searchParams = useStore($searchParams)

  useEffect(() => {
    logInGoogle(searchParams).then((possibleWorkspaceId) => {
      if (possibleWorkspaceId) {
        setCurrentWorkspace(possibleWorkspaceId)
      }
    })
  }, [searchParams])

  return <DialogLoadingOverlay />
}
