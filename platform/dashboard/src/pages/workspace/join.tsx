import { useEffect, useState } from 'react'
import { useStore } from '@nanostores/react'
import { CheckCircle2, XCircle } from 'lucide-react'

import { Button } from '~/elements/Button'

import { getInviteTokenFromURL } from '~/features/workspaces/stores/invitation'
import { openRoute } from '~/lib/router'
import { $inviteToken } from '~/features/workspaces/stores/invite.ts'
import { useAcceptInvitationMutation } from '~/features/workspaces/hooks/useWorkspaceMutations'

export function JoinWorkspacePage() {
  const inviteFromStore = useStore($inviteToken)
  const [token, setToken] = useState<string | null>(null)
  const { isPending: loading, data, error, mutateAsync: mutate } = useAcceptInvitationMutation()
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
          <XCircle className="mb-6 h-16 w-16 text-danger" />
          <h1 className="mb-4 text-2xl font-bold">Invalid Invitation Link</h1>
          <p className="mb-6 text-content2">
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
          <div className="mb-6 h-16 w-16 animate-spin rounded-full border-4 border-solid border-primary border-t-transparent"></div>
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
          <XCircle className="mb-6 h-16 w-16 text-danger" />
          <h1 className="mb-4 text-2xl font-bold">Invitation Error</h1>
          <p className="mb-6 text-content2">
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
        <CheckCircle2 className="mb-6 h-16 w-16 text-success" />
        <h1 className="mb-4 text-2xl font-bold">Invitation Accepted!</h1>
        <p className="mb-6 text-content2">
          You have successfully joined the workspace. Redirecting to dashboard...
        </p>
        <Button onClick={() => openRoute('home')}>Go to Dashboard</Button>
      </div>
    </div>
  )
}
