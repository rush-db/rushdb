import { useStore } from '@nanostores/react'
import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Check, KeyRound } from 'lucide-react'
import { TextField } from '~/elements/Input'
import { GithubIcon } from '~/elements/GithubIcon'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { $user, useUser } from '~/features/auth/stores/user'
import {
  useDeleteUserMutation,
  useSendRecoveryLinkMutation,
  useUpdateUserMutation
} from '~/features/auth/hooks/useAuthMutations'

import { Button } from '~/elements/Button.tsx'

import { ConfirmDialog } from '~/elements/ConfirmDialog.tsx'
import { isFreePlan } from '~/features/billing/utils.ts'
import { useCurrentWorkspacePlan } from '~/features/billing/hooks/useBillingHooks'
import { api } from '~/lib/api.ts'

import { useCurrentWorkspaceQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { useLeaveWorkspaceMutation } from '~/features/workspaces/hooks/useWorkspaceMutations'

import { ProfileSettingsLayout } from '~/features/auth/layout/ProfileSettingsLayout'
import { toast } from '~/elements/Toast'
import { ThemeSwitcher } from '~/features/auth/components/ThemeSwitcher'

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
        <h2 className="text-base font-semibold text-content">{title}</h2>
        {description && <p className="mt-1 text-sm leading-6 text-content2">{description}</p>}
      </div>
      {children && <div className="p-5">{children}</div>}
    </section>
  )
}

const profileNameSchema = z.object({
  firstName: z.string().trim().max(128, 'Keep it under 128 characters').default(''),
  lastName: z.string().trim().max(128, 'Keep it under 128 characters').default('')
})

function ProfileName() {
  const user = useUser()
  const { mutateAsync, isPending } = useUpdateUserMutation()

  const {
    formState: { errors, isDirty, isSubmitting, isValid },
    handleSubmit,
    register,
    reset
  } = useForm({
    defaultValues: { firstName: user.firstName ?? '', lastName: user.lastName ?? '' },
    resolver: zodResolver(profileNameSchema)
  })

  // The user store hydrates asynchronously; sync the form once it arrives/changes.
  useEffect(() => {
    reset({ firstName: user.firstName ?? '', lastName: user.lastName ?? '' })
  }, [reset, user.firstName, user.lastName])

  const onSubmit = handleSubmit(async ({ firstName, lastName }) => {
    try {
      await mutateAsync({ firstName, lastName })
      toast({ title: 'Profile updated' })
    } catch {
      toast({ title: 'Could not update profile', variant: 'danger' })
    }
  })

  return (
    <SettingsSection
      title="Name"
      description="Your name is shown across the dashboard and on workspace invitations."
    >
      <form className="flex flex-col gap-4" onSubmit={onSubmit}>
        <div className="grid gap-4 sm:grid-cols-2">
          <TextField label="First name" {...register('firstName')} error={errors.firstName?.message} />
          <TextField label="Last name" {...register('lastName')} error={errors.lastName?.message} />
        </div>
        <div className="flex justify-end gap-3">
          {isDirty && (
            <Button onClick={() => reset()} type="button" variant="secondary">
              Reset
            </Button>
          )}
          <Button
            disabled={!isDirty || !isValid}
            loading={isSubmitting || isPending}
            type="submit"
            variant="primary"
          >
            Save
          </Button>
        </div>
      </form>
    </SettingsSection>
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

function Appearance() {
  return (
    <SettingsSection
      title="Appearance"
      description="Choose how RushDB looks to you. Select a theme, or follow your system setting."
    >
      <ThemeSwitcher />
    </SettingsSection>
  )
}

const GoogleIcon = () => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    width="20"
    height="20"
    viewBox="0 0 30 30"
    fill="currentColor"
    aria-hidden
  >
    <path d="M 15.003906 3 C 8.3749062 3 3 8.373 3 15 C 3 21.627 8.3749062 27 15.003906 27 C 25.013906 27 27.269078 17.707 26.330078 13 L 25 13 L 22.732422 13 L 15 13 L 15 17 L 22.738281 17 C 21.848702 20.448251 18.725955 23 15 23 C 10.582 23 7 19.418 7 15 C 7 10.582 10.582 7 15 7 C 17.009 7 18.839141 7.74575 20.244141 8.96875 L 23.085938 6.1289062 C 20.951937 4.1849063 18.116906 3 15.003906 3 z"></path>
  </svg>
)

function SignInMethodRow({
  icon,
  name,
  description,
  connected
}: {
  icon: React.ReactNode
  name: string
  description: string
  connected: boolean
}) {
  return (
    <div className="flex items-center justify-between gap-4 py-4 first:pt-0 last:pb-0">
      <div className="flex items-center gap-3">
        <span
          className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-md border ${connected ? 'text-content' : 'text-content3'}`}
        >
          {icon}
        </span>
        <div>
          <div className="text-sm font-medium text-content">{name}</div>
          <div className="text-sm leading-6 text-content2">{description}</div>
        </div>
      </div>
      {connected ?
        <span className="inline-flex shrink-0 items-center gap-1 rounded-md bg-success/10 px-2 py-1 text-xs font-medium text-success">
          <Check className="h-3.5 w-3.5" /> Connected
        </span>
      : <span className="shrink-0 rounded-md border border-content3/30 px-2 py-1 text-xs font-medium text-content3">
          Not connected
        </span>
      }
    </div>
  )
}

function SignInMethods() {
  const user = useUser()

  // Email/password is available unless this is a pure external-identity account.
  const emailConnected = user.isEmail !== false

  const methods = [
    {
      name: 'Email & password',
      icon: <KeyRound className="h-5 w-5" />,
      description: 'Sign in with your email and a password.',
      connected: emailConnected
    },
    {
      name: 'Google',
      icon: <GoogleIcon />,
      description: 'Sign in with your Google account.',
      connected: Boolean(user.googleConnected)
    },
    {
      name: 'GitHub',
      icon: <GithubIcon size={20} />,
      description: 'Sign in with your GitHub account.',
      connected: Boolean(user.githubConnected)
    }
  ]

  return (
    <SettingsSection
      title="Sign-in methods"
      description={
        <>
          Ways you can sign in to RushDB. All linked methods share the same account because they use{' '}
          <span className="font-medium text-content">{user.login}</span>.
        </>
      }
    >
      <div className="divide-y">
        {methods.map((method) => (
          <SignInMethodRow
            connected={method.connected}
            description={method.description}
            icon={method.icon}
            key={method.name}
            name={method.name}
          />
        ))}
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
          <p className="mb-4 text-content">
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
    <section className="rounded-md border border-danger">
      <div className="border-b border-danger/40 p-5">
        <h2 className="text-base font-semibold text-danger">Danger Zone</h2>
        <p className="mt-1 text-sm leading-6 text-content2">
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
            <p className="mt-4 text-sm leading-6 text-content2">
              This action is irreversible. Once you delete your account, all your data will be permanently
              wiped and cannot be restored.
            </p>

            <div className="mt-4 rounded-md border border-danger p-4">
              <p className="text-sm font-semibold text-content">Warning:</p>
              <ul className="list-disc pl-5 text-sm leading-6 text-content2">
                <li>All your data will be permanently deleted.</li>
                <li>This action cannot be undone.</li>
                <li>You will lose access to all your information and services.</li>
              </ul>
            </div>
          </>
        )}

        {!isOwner && (
          <>
            <p className="mt-4 text-sm leading-6 text-content2">
              Your projects, data, and workspace settings will remain intact, but you will lose your access
              rights to view, edit, or add anything in this workspace.
            </p>

            <div className="mt-4 rounded-md border border-warning p-4">
              <p className="text-sm font-semibold text-content">Note:</p>
              <ul className="list-disc pl-5 text-sm leading-6 text-content2">
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
          <p className="text-sm leading-6 text-content2">
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
          <ProfileName />
          <Appearance />
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
          <p className="text-sm leading-6 text-content2">
            Manage sign-in, connected accounts, and authorized application access.
          </p>
        </div>
      </PageHeader>
      <PageContent contained>
        <div className="flex max-w-5xl flex-col gap-6">
          <SignInMethods />
          {user.isEmail !== false && <ChangePassword />}
        </div>
      </PageContent>
    </ProfileSettingsLayout>
  )
}
