import { useEffect, useState } from 'react'
import { useStore } from '@nanostores/react'
import { CheckCircle2, XCircle } from 'lucide-react'

import { Button } from '~/elements/Button'

import { acceptInvitation, getInviteTokenFromURL } from '~/features/workspaces/stores/invitation'
import { openRoute } from '~/lib/router'
import { $inviteToken } from '~/features/workspaces/stores/invite.ts'

export function JoinWorkspacePage() {
  const inviteFromStore = useStore($inviteToken)
  const [token, setToken] = useState<string | null>(null)
  const { loading, data, error, mutate } = useStore(acceptInvitation)
  const [processingInvite, setProcessingInvite] = useState(false)

  useEffect(() => {
    const token = getInviteTokenFromURL() ?? inviteFromStore

    setToken(token)

    if (!token) return
    ;(async () => {
      setProcessingInvite(true)
      try {
        await mutate({ token })
      } catch {
      } finally {
        setProcessingInvite(false)
        $inviteToken.set(null)
      }
    })()
  }, [mutate])

  // User landed on this page without an invite token
  if (!token) {
    return (
      <div>
        <div className="mx-auto flex max-w-md flex-1 flex-col items-center justify-center p-8 text-center">
          <XCircle className="text-danger mb-6 h-16 w-16" />
          <h1 className="mb-4 text-2xl font-bold">Invalid Invitation Link</h1>
          <p className="text-content2 mb-6">
            The workspace invitation link appears to be invalid or missing. Please check the link and try
            again, or contact the workspace administrator.
          </p>
          <Button onClick={() => openRoute('home')}>Go to Dashboard</Button>
        </div>
      </div>
    )
  }

  // Still processing
  if (processingInvite && loading) {
    return (
      <div>
        <div className="mx-auto flex max-w-md flex-1 flex-col items-center justify-center p-8 text-center">
          <div className="border-primary mb-6 h-16 w-16 animate-spin rounded-full border-4 border-solid border-t-transparent"></div>
          <h1 className="mb-4 text-2xl font-bold">Accepting Invitation</h1>
          <p className="text-content2">Please wait while we process your workspace invitation...</p>
        </div>
      </div>
    )
  }

  // Error state
  if (error) {
    return (
      <div>
        <div className="mx-auto flex max-w-md flex-1 flex-col items-center justify-center p-8 text-center">
          <XCircle className="text-danger mb-6 h-16 w-16" />
          <h1 className="mb-4 text-2xl font-bold">Invitation Error</h1>
          <p className="text-content2 mb-6">
            {error instanceof Error ?
              error.message
            : 'There was a problem accepting this workspace invitation. The token may be expired or invalid.'}
          </p>
          <Button onClick={() => openRoute('home')}>Go to Dashboard</Button>
        </div>
      </div>
    )
  }

  // Success state (although user should be redirected by this point)
  return (
    <div>
      <div className="mx-auto flex max-w-md flex-1 flex-col items-center justify-center p-8 text-center">
        <CheckCircle2 className="text-success mb-6 h-16 w-16" />
        <h1 className="mb-4 text-2xl font-bold">Invitation Accepted!</h1>
        <p className="text-content2 mb-6">
          You have successfully joined the workspace. Redirecting to dashboard...
        </p>
        <Button onClick={() => openRoute('home')}>Go to Dashboard</Button>
      </div>
    </div>
  )
}
