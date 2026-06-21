import { useStore } from '@nanostores/react'
import { TextField } from '~/elements/Input'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { $user, useUser } from '~/features/auth/stores/user'
import { useDeleteUserMutation, useSendRecoveryLinkMutation } from '~/features/auth/hooks/useAuthMutations'

import { Button } from '~/elements/Button.tsx'

import { ConfirmDialog } from '~/elements/ConfirmDialog.tsx'
import { isFreePlan } from '~/features/billing/utils.ts'
import { useCurrentWorkspacePlan } from '~/features/billing/hooks/useBillingHooks'
import { api } from '~/lib/api.ts'

import { useCurrentWorkspaceQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { useLeaveWorkspaceMutation } from '~/features/workspaces/hooks/useWorkspaceMutations'

import { ProfileSettingsLayout } from '~/features/auth/layout/ProfileSettingsLayout'
import { toast } from '~/elements/Toast'

function SettingsSection({
  children,
  description,
  title
}: {
  children?: React.ReactNode
  description?: React.ReactNode
  title: React.ReactNode
}) {
  return (
    <section className="rounded-md border">
      <div className="border-b p-5">
        <h2 className="text-content text-base font-semibold">{title}</h2>
        {description && <p className="text-content2 mt-1 text-sm leading-6">{description}</p>}
      </div>
      {children && <div className="p-5">{children}</div>}
    </section>
  )
}

function ChangePassword() {
  const user = useUser()
  const { mutateAsync, isPending } = useSendRecoveryLinkMutation()

  const sendRecoveryLink = async () => {
    try {
      await mutateAsync({ email: user.login })
      toast({
        description: 'Check your inbox for the password reset link.',
        title: 'Recovery email sent'
      })
    } catch {
      toast({
        description: "Couldn't send a recovery link to this account.",
        title: 'Password reset failed',
        variant: 'danger'
      })
    }
  }

  return (
    <SettingsSection
      title="Change Password"
      description="RushDB sends a secure recovery link to your login email. Follow that link to set a new password."
    >
      <Button loading={isPending} onClick={sendRecoveryLink} variant="secondary">
        Send recovery link
      </Button>
    </SettingsSection>
  )
}

function ConnectedAccounts() {
  const user = useUser()

  if (user.isEmail !== false) {
    return null
  }

  return (
    <SettingsSection
      title="Connected Accounts"
      description="This account signs in through an external identity provider. Password changes are managed by that provider."
    >
      <div className="flex items-center justify-between gap-4">
        <div>
          <div className="text-content text-sm font-medium">{user.login}</div>
          <div className="text-content2 text-sm leading-6">External sign-in account</div>
        </div>
        <span className="bg-success/10 text-success rounded px-2 py-1 text-xs font-medium">Connected</span>
      </div>
    </SettingsSection>
  )
}

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
            try {
              const { redirectUrl } = await api.billing.createPortalSession({
                returnUrl: window.location.href
              })
              if (redirectUrl) {
                window.location.replace(redirectUrl)
              }
            } catch {
              toast({
                title: "Couldn't open the billing portal",
                description:
                  'We couldn’t find an active subscription to manage. Please refresh, or contact support if this keeps happening.',
                variant: 'danger'
              })
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
    <section className="border-danger rounded-md border">
      <div className="border-danger/40 border-b p-5">
        <h2 className="text-danger text-base font-semibold">Danger Zone</h2>
        <p className="text-content2 mt-1 text-sm leading-6">
          {isOwner ?
            'Deleting your account permanently removes your account and all data owned by it.'
          : 'Leaving this workspace revokes your access while keeping workspace data intact.'}
        </p>
      </div>
      <div className="p-5">
        <div>
          <DeleteAccount />
        </div>

        {isOwner && (
          <>
            <p className="text-content2 mt-4 text-sm leading-6">
              This action is irreversible. Once you delete your account, all your data will be permanently
              wiped and cannot be restored.
            </p>

            <div className="border-danger mt-4 rounded border p-4">
              <p className="text-content text-sm font-semibold">Warning:</p>
              <ul className="text-content2 list-disc pl-5 text-sm leading-6">
                <li>All your data will be permanently deleted.</li>
                <li>This action cannot be undone.</li>
                <li>You will lose access to all your information and services.</li>
              </ul>
            </div>
          </>
        )}

        {!isOwner && (
          <>
            <p className="text-content2 mt-4 text-sm leading-6">
              Your projects, data, and workspace settings will remain intact, but you will lose your access
              rights to view, edit, or add anything in this workspace.
            </p>

            <div className="border-warning mt-4 rounded border p-4">
              <p className="text-content text-sm font-semibold">Note:</p>
              <ul className="text-content2 list-disc pl-5 text-sm leading-6">
                <li>You will be returned to your personal workspace.</li>
                <li>You cannot rejoin this workspace unless an owner invites you again.</li>
              </ul>
            </div>
          </>
        )}
      </div>
    </section>
  )
}

export function ProfilePage() {
  const user = useUser()
  return (
    <ProfileSettingsLayout>
      <PageHeader contained>
        <div className="flex max-w-3xl flex-col gap-2">
          <PageTitle>Profile</PageTitle>
          <p className="text-content2 text-sm leading-6">
            Manage your account identity and account-level settings.
          </p>
        </div>
      </PageHeader>
      <PageContent contained>
        <div className="flex max-w-5xl flex-col gap-6">
          <SettingsSection
            title="Login"
            description="This email is used to identify your RushDB account and receive account messages."
          >
            <TextField readOnly disabled value={user.login} />
          </SettingsSection>
          <DangerZone />
        </div>
      </PageContent>
    </ProfileSettingsLayout>
  )
}

export function ProfileSecurityPage() {
  const user = useUser()

  return (
    <ProfileSettingsLayout>
      <PageHeader contained>
        <div className="flex max-w-3xl flex-col gap-2">
          <PageTitle>Security</PageTitle>
          <p className="text-content2 text-sm leading-6">
            Manage sign-in, connected accounts, and authorized application access.
          </p>
        </div>
      </PageHeader>
      <PageContent contained>
        <div className="flex max-w-5xl flex-col gap-6">
          {user.isEmail !== false && <ChangePassword />}
          <ConnectedAccounts />
        </div>
      </PageContent>
    </ProfileSettingsLayout>
  )
}
