import { useStore } from '@nanostores/react'
import { UsersIcon, X } from 'lucide-react'
import {
  useEffect,
  useState,
  FormEvent,
  HTMLAttributes,
  ThHTMLAttributes,
  TdHTMLAttributes,
  useMemo
} from 'react'
import type { SubmitHandler } from 'react-hook-form'

import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import { Banner } from '~/elements/Banner'
import { Setting, SettingsList } from '~/elements/Setting'
import { TextField } from '~/elements/Input'
import { Skeleton } from '~/elements/Skeleton'
import { Button } from '~/elements/Button'
import { Checkbox } from '~/elements/Checkbox'
import { ConfirmDialog } from '~/elements/ConfirmDialog'
import { TableRow } from '~/elements/Table'
import { cn } from '~/lib/utils'

import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace'
import { $workspaceProjects } from '~/features/workspaces/stores/projects'
import {
  $workspaceUsers,
  $workspaceAccessList,
  inviteUser,
  revokeAccess,
  $workspacePendingInvites,
  removePendingInvite
} from '~/features/workspaces/stores/users'
import { object, string, useForm } from '~/lib/form'
import { PendingInvite } from '~/features/workspaces/types.ts'
import { getRoutePath } from '~/lib/router.ts'
import { $platformSettings } from '~/features/auth/stores/settings.ts'
import { IconButton } from '~/elements/IconButton'

// Custom table components
function Table({ children, className, ...props }: HTMLAttributes<HTMLTableElement>) {
  return (
    <div className="w-full overflow-auto">
      <table className={cn('w-full caption-bottom text-sm', className)} {...props}>
        {children}
      </table>
    </div>
  )
}

function TableHeader({ children, className, ...props }: HTMLAttributes<HTMLTableSectionElement>) {
  return (
    <thead className={cn('bg-fill3 [&_tr]:border-b', className)} {...props}>
      {children}
    </thead>
  )
}

function TableHead({ children, className, ...props }: ThHTMLAttributes<HTMLTableCellElement>) {
  return (
    <th className={cn('text-content2 px-4 py-3 text-left font-medium', className)} {...props}>
      {children}
    </th>
  )
}

function TableBody({ children, className, ...props }: HTMLAttributes<HTMLTableSectionElement>) {
  return (
    <tbody className={cn('[&_tr:last-child]:border-0', className)} {...props}>
      {children}
    </tbody>
  )
}

function TableCell({ children, className, ...props }: TdHTMLAttributes<HTMLTableCellElement>) {
  return (
    <td className={cn('px-4 py-3', className)} {...props}>
      {children}
    </td>
  )
}

// Form validation schema for inviting users
const inviteSchema = object({
  email: string().email('Must be a valid email').required('Email is required')
})

interface InviteFormValues {
  email: string
}

function InviteUserSetting() {
  const [projectSelections, setProjectSelections] = useState<Record<string, boolean>>({})
  const { data: projects, loading: projectsLoading } = useStore($workspaceProjects)
  const { data: workspace } = useStore($currentWorkspace)

  // Form handling for user invitation
  const {
    formState: { errors, isDirty, isSubmitting, isValid },
    handleSubmit,
    register,
    reset
  } = useForm<InviteFormValues>({
    schema: inviteSchema
  })

  const { mutate } = useStore(inviteUser)

  // Reset project selections when projects change
  useEffect(() => {
    if (projects) {
      const selections = Object.fromEntries(projects.map((p) => [p.id, false]))
      setProjectSelections(selections)
    }
  }, [projects])

  const handleInvite: SubmitHandler<InviteFormValues> = async (values) => {
    if (!workspace) return

    const selectedProjectIds = Object.entries(projectSelections)
      .filter(([_, selected]) => selected)
      .map(([id]) => id)

    await mutate({
      id: workspace.id,
      email: values.email,
      projectIds: selectedProjectIds.length ? selectedProjectIds : undefined
    })

    // Reset form after successful submission
    reset()
    setProjectSelections(Object.fromEntries(Object.keys(projectSelections).map((id) => [id, false])))
  }

  return (
    <Setting
      onReset={(event: FormEvent<HTMLFormElement>) => {
        event.preventDefault()
        reset()
        setProjectSelections(Object.fromEntries(Object.keys(projectSelections).map((id) => [id, false])))
      }}
      title="Invite a user"
      description="Invite a user to your workspace by email. You can select which projects they will have access to."
      isDirty={isDirty}
      isSubmitting={isSubmitting}
      isValid={isValid}
      onSubmit={handleSubmit(handleInvite)}
    >
      <div className="flex flex-col gap-4">
        <TextField
          {...register('email')}
          error={errors.email?.message as string}
          placeholder="user@example.com"
          className="w-full"
        />

        {projectsLoading ?
          <div className="flex gap-2">
            <Skeleton className="h-5 w-5" />
            <Skeleton className="h-5 flex-1" />
          </div>
        : projects && projects.length > 0 ?
          <div className="mt-2">
            <div className="text-content mb-2 text-sm font-medium">Grant access to projects:</div>
            <div className="space-y-2">
              {projects.map((project) => (
                <div key={project.id} className="flex items-center gap-2">
                  <Checkbox
                    checked={projectSelections[project.id] || false}
                    onCheckedChange={(checked) => {
                      setProjectSelections((prev) => ({
                        ...prev,
                        [project.id]: !!checked
                      }))
                    }}
                    id={`project-${project.id}`}
                  />
                  <label htmlFor={`project-${project.id}`} className="text-content cursor-pointer text-sm">
                    {project.name}
                  </label>
                </div>
              ))}
            </div>
          </div>
        : null}
      </div>
    </Setting>
  )
}

function UserTable() {
  const { data: users, loading } = useStore($workspaceUsers)
  const { data: accessList } = useStore($workspaceAccessList)
  const { data: projects } = useStore($workspaceProjects)
  const { data: workspace } = useStore($currentWorkspace)
  const { mutate: revokeUserAccess } = useStore(revokeAccess)

  if (loading) {
    return (
      <div className="mt-6 flex flex-1 items-center justify-center">
        <Skeleton className="h-60 w-full rounded-md" />
      </div>
    )
  }

  if (!users || users.length === 0) {
    return (
      <Banner className="mt-6">
        No users have been invited to this workspace yet. Invite users using the form above.
      </Banner>
    )
  }

  const handleRevoke = (userId: string): Promise<unknown> => {
    if (!workspace) return Promise.resolve(undefined)

    return revokeUserAccess({
      id: workspace.id,
      userIds: [userId]
    })
  }

  return (
    <div className="mt-6">
      <div className="text-content mb-4 text-lg font-medium">Workspace Users</div>

      <Table className="border-b">
        <TableHeader>
          <TableRow>
            <TableHead>Email</TableHead>
            <TableHead>Workspace Role</TableHead>
            <TableHead>Project Access</TableHead>
            <TableHead className="w-[100px]">Actions</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {users.map((user) => (
            <TableRow key={user.id}>
              <TableCell className="font-medium">{user.login}</TableCell>
              <TableCell className="font-medium">{user.role}</TableCell>
              <TableCell>
                {projects && user.role !== 'owner' && accessList && (
                  <div className="flex flex-wrap gap-1">
                    {projects
                      .filter((project) => {
                        // Check if user has access to this project
                        return accessList[project.id]?.includes(user.id)
                      })
                      .map((project) => (
                        <div key={project.id} className="bg-fill2 rounded px-2 py-1 text-xs">
                          {project.name}
                        </div>
                      ))}
                    {!projects.some((project) => accessList[project.id]?.includes(user.id)) && (
                      <span className="text-content2 text-xs italic">No project access</span>
                    )}
                  </div>
                )}
              </TableCell>
              <TableCell>
                <ConfirmDialog
                  description="Are you sure you want to revoke this user's access? They will no longer be able to access the workspace."
                  handler={() => handleRevoke(user.id)}
                  title="Revoke User Access"
                  trigger={
                    user.role !== 'owner' && (
                      <IconButton size="small" variant="ghost" aria-label={'Revoke User Access'}>
                        <X />
                      </IconButton>
                    )
                  }
                />
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  )
}

export function PendingInvitesTable() {
  const { data: invites, loading } = useStore($workspacePendingInvites)
  const { data: workspace } = useStore($currentWorkspace)
  const { mutate: removeInvite } = useStore(removePendingInvite)

  const handleRemoveInvite = (email: string): Promise<unknown> => {
    if (!workspace) return Promise.resolve(undefined)

    return removeInvite({
      id: workspace.id,
      email
    })
  }

  if (loading) {
    return <div className="mt-6">Loading pending invites…</div>
  }
  if (!invites || invites.length === 0) {
    return (
      <div className="mt-8">
        <div className="text-content mb-4 text-lg font-medium">Pending Invites</div>
        <div className="text-content2 mt-4 italic">No pending invites.</div>
      </div>
    )
  }

  return (
    <div className="mt-8">
      <div className="text-content mb-4 text-lg font-medium">Pending Invites</div>
      <Table className="border-b">
        <TableHeader>
          <TableRow>
            <TableHead>Email</TableHead>
            <TableHead>Date Sent</TableHead>
            <TableHead className="w-[100px]">Actions</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {invites.map((invite: PendingInvite) => (
            <TableRow key={invite.email}>
              <TableCell className="font-medium">{invite.email}</TableCell>
              <TableCell>
                {new Date(invite.createdAt).toLocaleString(undefined, {
                  year: 'numeric',
                  month: 'short',
                  day: 'numeric',
                  hour: '2-digit',
                  minute: '2-digit'
                })}
              </TableCell>
              <TableCell>
                <ConfirmDialog
                  title="Remove Pending Invite"
                  description={`Remove invitation for ${invite.email}?`}
                  handler={() => handleRemoveInvite(invite.email)}
                  trigger={
                    <IconButton size="small" variant="ghost" aria-label={'Revoke User Access'}>
                      <X />
                    </IconButton>
                  }
                />
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  )
}

export function WorkspaceUsersPage() {
  const { data: workspace } = useStore($currentWorkspace)
  const userLimit = workspace?.limits?.users
  const { data: users } = useStore($workspaceUsers)
  const { data: invites } = useStore($workspacePendingInvites)
  const { loading, data: platformSettings } = useStore($platformSettings)

  const usersCount = useMemo(() => {
    return (users?.length || 1) + (invites?.length || 0)
  }, [invites, users])

  return (
    <WorkspacesLayout>
      <PageHeader contained>
        <PageTitle>Workspace Users</PageTitle>
        <div className="text-content2 flex items-center gap-1">
          <UsersIcon className="h-4 w-4" />
          {platformSettings?.selfHosted ?
            <span>
              {usersCount} user{usersCount > 1 ? 's' : ''}
            </span>
          : <span>
              {usersCount} / {userLimit} users
            </span>
          }
        </div>
      </PageHeader>
      <PageContent contained>
        <div className="flex flex-1 flex-col">
          {!loading && userLimit && usersCount >= userLimit ?
            !platformSettings?.selfHosted ?
              <Banner
                className="mb-6 min-h-[150px]"
                title={`You've reached your ${userLimit} active user limit. Additional users will be billed according to your plan.`}
                action={
                  <Button as="a" href={getRoutePath('workspaceBilling')} variant="accent">
                    Go to Billing
                  </Button>
                }
              />
            : null
          : null}

          {usersCount === userLimit && !workspace?.planId ? null : (
            <>
              <SettingsList>
                <InviteUserSetting />
              </SettingsList>

              <UserTable />
              <PendingInvitesTable />
            </>
          )}
        </div>
      </PageContent>
    </WorkspacesLayout>
  )
}
