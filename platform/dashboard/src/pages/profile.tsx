import { useStore } from '@nanostores/react'

import { TextField } from '~/elements/Input'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Setting } from '~/elements/Setting'
import { $user, useUser } from '~/features/auth/stores/user'
import { useDeleteUserMutation } from '~/features/auth/hooks/useAuthMutations'

import { Button } from '~/elements/Button.tsx'

import { ConfirmDialog } from '~/elements/ConfirmDialog.tsx'
import { isFreePlan } from '~/features/billing/utils.ts'
import { useCurrentWorkspacePlan } from '~/features/billing/hooks/useBillingHooks'
import { api } from '~/lib/api.ts'

import { useCurrentWorkspaceQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { useLeaveWorkspaceMutation } from '~/features/workspaces/hooks/useWorkspaceMutations'

import { Divider } from '~/elements/Divider.tsx'
import { openRoute } from '~/lib/router'

function DeleteAccount() {
  const { mutateAsync: deleteAccount } = useDeleteUserMutation()
  const { mutateAsync: leave } = useLeaveWorkspaceMutation()
  const { currentPlan } = useCurrentWorkspacePlan()
  const paidUser = currentPlan && !isFreePlan(currentPlan)
  const { data: workspace } = useCurrentWorkspaceQuery()
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
            return deleteAccount()
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

        {!isOwner && (
          <>
            <p className="text-content mt-4">
              Your projects, data, and workspace settings will remain intact, but you will lose your access
              rights to view, edit, or add anything in this workspace.
            </p>

            <div className="border-warning mt-4 rounded border p-4">
              <p className="font-semibold">Note:</p>
              <ul className="text-content list-disc pl-5">
                <li>You will be returned to your personal workspace.</li>
                <li>You cannot rejoin this workspace unless an owner invites you again.</li>
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
          <Setting
            title="Connected Apps"
            description="Manage third-party applications that have access to your RushDB data via OAuth."
            button={
              <Button onClick={() => openRoute('workspaceSettings')} variant="secondary" size="small">
                Manage Apps
              </Button>
            }
          />
        </ul>
        <ul>
          <DangerZone />
        </ul>
      </PageContent>
    </>
  )
}
