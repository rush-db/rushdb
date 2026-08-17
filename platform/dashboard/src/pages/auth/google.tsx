import { useStore } from '@nanostores/react'
import { useEffect, useRef } from 'react'

import { logInGoogle } from '~/features/auth/stores/auth'
import { $searchParams } from '~/lib/router'
import { DialogLoadingOverlay } from '~/elements/Dialog.tsx'
import { setCurrentWorkspace } from '~/features/workspaces/stores/current-workspace.ts'

export function AuthGoogle() {
  const searchParams = useStore($searchParams)
  const exchangedCode = useRef<string | undefined>(undefined)

  useEffect(() => {
    const code = searchParams.code

    if (!code || exchangedCode.current === code) {
      return
    }

    exchangedCode.current = code

    logInGoogle(searchParams).then((possibleWorkspaceId) => {
      if (possibleWorkspaceId) {
        setCurrentWorkspace(possibleWorkspaceId)
      }
    })
  }, [searchParams])

  return <DialogLoadingOverlay />
}
