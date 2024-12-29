import { useStore } from '@nanostores/react'

import { TextField } from '~/elements/Input'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Setting } from '~/elements/Setting'
import { deleteUser, useUser } from '~/features/auth/stores/user'

import { Button } from '~/elements/Button.tsx'

import { ConfirmDialog } from '~/elements/ConfirmDialog.tsx'
import { $paidUser } from '~/features/billing/stores/plans.ts'
import { api } from '~/lib/api.ts'

import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace.ts'

import { Divider } from '~/elements/Divider.tsx'

function DeleteAccount() {
  const { mutate } = useStore(deleteUser)
  const paidUser = useStore($paidUser)
  const { data: workspace } = useStore($currentWorkspace)

  const isSubscriptionCanceled = workspace?.isSubscriptionCancelled

  return (
    <>
      {paidUser && !isSubscriptionCanceled ? (
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
          <p className="mb-4 text-content">
            To proceed with deleting your account, please cancel your
            subscription first. Once the subscription is canceled, you can
            delete your account.
          </p>
          <Button variant="danger" as="button" type="submit">
            Cancel Subscription
          </Button>
        </form>
      ) : null}
      <ConfirmDialog
        handler={() => mutate({})}
        trigger={
          <Button
            variant="danger"
            disabled={paidUser && !isSubscriptionCanceled}
          >
            Delete Account Permanently
          </Button>
        }
        description={`This action is irreversible. Once you delete your account, all your data will be permanently wiped and cannot be restored. Are you sure you want to continue?`}
        title={`Delete Account Permanently`}
      />
    </>
  )
}

function DangerZone() {
  return (
    <div className="mx-auto">
      <div className="mt-6 rounded ">
        <h2 className="mb-6 text-2xl font-semibold text-danger">Danger Zone</h2>

        <Divider />

        <div className="mt-6">
          <DeleteAccount />
        </div>

        <p className="mt-4 text-content">
          This action is irreversible. Once you delete your account, all your
          data will be permanently wiped and cannot be restored.
        </p>

        <div className="mt-4 rounded border border-danger p-4 ">
          <p className="font-semibold">Warning:</p>
          <ul className="list-disc pl-5 text-content">
            <li>All your data will be permanently deleted.</li>
            <li>This action cannot be undone.</li>
            <li>You will lose access to all your information and services.</li>
          </ul>
        </div>
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
