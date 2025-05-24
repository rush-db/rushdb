import { useStore } from '@nanostores/react'

import { TextField } from '~/elements/Input'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Setting } from '~/elements/Setting'
import { $user, deleteUser, useUser } from '~/features/auth/stores/user'

import { Button } from '~/elements/Button.tsx'

import { ConfirmDialog } from '~/elements/ConfirmDialog.tsx'
import { $paidUser } from '~/features/billing/stores/plans.ts'
import { api } from '~/lib/api.ts'

import { $currentWorkspace, leaveWorkspace } from '~/features/workspaces/stores/current-workspace.ts'

import { Divider } from '~/elements/Divider.tsx'

function DeleteAccount() {
  const { mutate: deleteAccount } = useStore(deleteUser)
  const { mutate: leave } = useStore(leaveWorkspace)
  const paidUser = useStore($paidUser)
  const { data: workspace } = useStore($currentWorkspace)
  const currentUser = useStore($user)
  const isOwner = currentUser.currentScope?.role === 'owner'

  const isSubscriptionCanceled = workspace?.isSubscriptionCancelled
  const title = isOwner ? 'Confirm Account Deletion' : 'Confirm Leave Workspace'
  const description =
    isOwner ?
      'Deleting your account will permanently remove all your data and settings. This action cannot be undone. Continue?'
    : 'Leaving this workspace will revoke your access. Are you sure you want to continue?'

  return (
    <>
      {paidUser && !isSubscriptionCanceled && isOwner ?
        <form
          onSubmit={async (event) => {
            event.preventDefault()
            const { redirectUrl } = await api.billing.createPortalSession({
              returnUrl: window.location.href
            })
            if (redirectUrl) {
              window.location.replace(redirectUrl)
            }
          }}
          className="mb-4"
        >
          <p className="text-content mb-4">
            To proceed with deleting your account, please cancel your subscription first. Once the
            subscription is canceled, you can delete your account.
          </p>
          <Button variant="danger" as="button" type="submit">
            Cancel Subscription
          </Button>
        </form>
      : null}
      <ConfirmDialog
        handler={() => {
          if (isOwner) {
            return deleteAccount({})
          } else if (workspace?.id) {
            return leave({ id: workspace.id })
          }
        }}
        trigger={
          isOwner ?
            <Button variant="danger" disabled={paidUser && !isSubscriptionCanceled}>
              Delete Account Permanently
            </Button>
          : <Button variant="danger">Leave Workspace</Button>
        }
        title={title}
        description={description}
      />
    </>
  )
}

function DangerZone() {
  const currentUser = useStore($user)
  const isOwner = currentUser.currentScope?.role === 'owner'

  return (
    <div className="mx-auto">
      <div className="mt-6 rounded">
        <h2 className="text-danger mb-6 text-2xl font-semibold">Danger Zone</h2>

        <Divider />

        <div className="mt-6">
          <DeleteAccount />
        </div>

        {isOwner && (
          <>
            <p className="text-content mt-4">
              This action is irreversible. Once you delete your account, all your data will be permanently
              wiped and cannot be restored.
            </p>

            <div className="border-danger mt-4 rounded border p-4">
              <p className="font-semibold">Warning:</p>
              <ul className="text-content list-disc pl-5">
                <li>All your data will be permanently deleted.</li>
                <li>This action cannot be undone.</li>
                <li>You will lose access to all your information and services.</li>
              </ul>
            </div>
          </>
        )}
      </div>
    </div>
  )
}

export function ProfilePage() {
  const user = useUser()
  return (
    <>
      <PageHeader contained>
        <PageTitle>Profile</PageTitle>
      </PageHeader>
      <PageContent contained>
        <ul>
          <Setting title="Login" readOnly={true}>
            <TextField readOnly disabled value={user.login} />
          </Setting>
        </ul>
        <ul>
          <DangerZone />
        </ul>
      </PageContent>
    </>
  )
}
