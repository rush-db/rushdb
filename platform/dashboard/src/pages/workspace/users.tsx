import { Sparkles, UserPlus, UsersIcon, X } from 'lucide-react'
import type { FormEvent, HTMLAttributes, ThHTMLAttributes, TdHTMLAttributes } from 'react'
import { useEffect, useState, useMemo } from 'react'
import type { SubmitHandler } from 'react-hook-form'

import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import { WorkspaceSettingsLayout } from '~/features/workspaces/layout/WorkspaceSettingsLayout'
import { Banner } from '~/elements/Banner'
import { TextField } from '~/elements/Input'
import { Skeleton } from '~/elements/Skeleton'
import { Button } from '~/elements/Button'
import { Checkbox } from '~/elements/Checkbox'
import { ConfirmDialog } from '~/elements/ConfirmDialog'
import { TableRow } from '~/elements/Table'
import { cn } from '~/lib/utils'
import { Dialog, DialogFooter, DialogTitle } from '~/elements/Dialog'

import {
  useCurrentWorkspaceQuery,
  useWorkspaceProjectsQuery,
  useWorkspaceUsersQuery,
  useWorkspaceAccessListQuery,
  useWorkspacePendingInvitesQuery
} from '~/features/workspaces/hooks/useWorkspaceQueries'
import {
  useInviteUserMutation,
  useRevokeAccessMutation,
  useRemovePendingInviteMutation
} from '~/features/workspaces/hooks/useWorkspaceMutations'
import { object, string, useForm } from '~/lib/form'
import type { PendingInvite } from '~/features/workspaces/types.ts'
import { getRoutePath } from '~/lib/router.ts'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
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

function InviteUserDialog({ disabled }: { disabled?: boolean }) {
  const [open, setOpen] = useState(false)
  const [projectSelections, setProjectSelections] = useState<Record<string, boolean>>({})
  const { data: projects, isPending: projectsLoading } = useWorkspaceProjectsQuery()
  const { data: workspace } = useCurrentWorkspaceQuery()

  // Form handling for user invitation
  const {
    formState: { errors, isDirty, isSubmitting, isValid },
    handleSubmit,
    register,
    reset
  } = useForm<InviteFormValues>({
    schema: inviteSchema
  })

  const { mutateAsync: mutate } = useInviteUserMutation()

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
      email: values.email,
      projectIds: selectedProjectIds.length ? selectedProjectIds : undefined
    })

    // Reset form after successful submission
    reset()
    setProjectSelections(Object.fromEntries(Object.keys(projectSelections).map((id) => [id, false])))
    setOpen(false)
  }

  return (
    <Dialog
      className="sm:max-w-xl"
      open={open}
      onOpenChange={(nextOpen) => {
        setOpen(nextOpen)

        if (!nextOpen) {
          reset()
          setProjectSelections(Object.fromEntries(Object.keys(projectSelections).map((id) => [id, false])))
        }
      }}
      trigger={
        <Button disabled={disabled} variant="accent">
          <UserPlus />
          Add member
        </Button>
      }
    >
      <form
        className="flex flex-col gap-5"
        onReset={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault()
          setOpen(false)
        }}
        onSubmit={handleSubmit(handleInvite)}
      >
        <div className="flex flex-col gap-2 pr-8">
          <DialogTitle>Invite a member</DialogTitle>
          <p className="text-content2 text-sm leading-6">
            Add someone to this workspace, then choose which projects they can open.
          </p>
        </div>

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
            <div className="text-content mb-2 text-sm font-medium">Grant project access:</div>
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
        <DialogFooter className="mt-1">
          <Button type="reset" variant="outline">
            Cancel
          </Button>
          <Button disabled={!isDirty || !isValid} loading={isSubmitting} type="submit" variant="accent">
            Send invite
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  )
}

function MembersTable() {
  const { data: users, isPending: loading } = useWorkspaceUsersQuery()
  const { data: invites, isPending: invitesLoading } = useWorkspacePendingInvitesQuery()
  const { data: accessList } = useWorkspaceAccessListQuery()
  const { data: projects } = useWorkspaceProjectsQuery()
  const { data: workspace } = useCurrentWorkspaceQuery()
  const { mutateAsync: revokeUserAccess } = useRevokeAccessMutation()
  const { mutateAsync: removeInvite } = useRemovePendingInviteMutation()

  if (loading || invitesLoading) {
    return (
      <div className="mt-6 flex flex-1 items-center justify-center">
        <Skeleton className="h-60 w-full rounded-md" />
      </div>
    )
  }

  if ((!users || users.length === 0) && (!invites || invites.length === 0)) {
    return <Banner className="mt-6">No members have been added to this workspace yet.</Banner>
  }

  const activeUsers = users ?? []
  const pendingInvites = invites ?? []

  const handleRevoke = (userId: string): Promise<unknown> => {
    if (!workspace) return Promise.resolve(undefined)

    return revokeUserAccess({
      userIds: [userId]
    })
  }

  const handleRemoveInvite = (email: string): Promise<unknown> => {
    if (!workspace) return Promise.resolve(undefined)

    return removeInvite({ email })
  }

  return (
    <div className="mt-6">
      <Table className="border-b">
        <TableHeader>
          <TableRow>
            <TableHead>Member</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>Workspace Role</TableHead>
            <TableHead>Project Access</TableHead>
            <TableHead>Invited</TableHead>
            <TableHead className="w-[100px]">Actions</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {activeUsers.map((user) => (
            <TableRow key={user.id}>
              <TableCell className="font-medium">{user.login}</TableCell>
              <TableCell>
                <span className="bg-success/10 text-success rounded px-2 py-1 text-xs font-medium">
                  Active
                </span>
              </TableCell>
              <TableCell className="font-medium">{user.role}</TableCell>
              <TableCell>
                {user.role === 'owner' ?
                  <span className="text-content2 text-xs">All projects</span>
                : projects &&
                  accessList && (
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
                  )
                }
              </TableCell>
              <TableCell>
                <span className="text-content2 text-xs">-</span>
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
          {pendingInvites.map((invite: PendingInvite) => (
            <TableRow key={invite.email}>
              <TableCell className="font-medium">{invite.email}</TableCell>
              <TableCell>
                <span className="bg-warning/10 text-warning rounded px-2 py-1 text-xs font-medium">
                  Pending
                </span>
              </TableCell>
              <TableCell>
                <span className="text-content2 text-xs">Invited</span>
              </TableCell>
              <TableCell>
                <span className="text-content2 text-xs">Pending acceptance</span>
              </TableCell>
              <TableCell>
                <span className="text-content2 text-xs">
                  {new Date(invite.createdAt).toLocaleString(undefined, {
                    year: 'numeric',
                    month: 'short',
                    day: 'numeric',
                    hour: '2-digit',
                    minute: '2-digit'
                  })}
                </span>
              </TableCell>
              <TableCell>
                <ConfirmDialog
                  title="Remove Pending Invite"
                  description={`Remove invitation for ${invite.email}?`}
                  handler={() => handleRemoveInvite(invite.email)}
                  trigger={
                    <IconButton size="small" variant="ghost" aria-label={'Remove pending invite'}>
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
  const { data: workspace } = useCurrentWorkspaceQuery()
  const userLimit = workspace?.userLimit
  const { data: users } = useWorkspaceUsersQuery()
  const { data: invites } = useWorkspacePendingInvitesQuery()
  const { data: platformSettings, isPending: loading } = usePlatformSettings()

  const usersCount = useMemo(() => {
    return (users?.length || 1) + (invites?.length || 0)
  }, [invites, users])

  return (
    <WorkspacesLayout>
      <WorkspaceSettingsLayout>
        <PageHeader className="items-start justify-between gap-5" contained>
          <div className="flex max-w-3xl flex-col gap-2">
            <PageTitle>Team Members</PageTitle>
            <p className="text-content2 text-sm leading-6">
              Manage who can work inside this workspace and which isolated projects they can access. Workspace
              owners manage membership; project permissions decide which project data a user can view or
              change.
            </p>
          </div>
          <div className="flex items-center gap-4">
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
            {usersCount === userLimit && !workspace?.planId ? null : (
              <InviteUserDialog
                disabled={!platformSettings?.selfHosted && Boolean(userLimit && usersCount >= userLimit)}
              />
            )}
          </div>
        </PageHeader>
        <PageContent contained>
          <div className="flex flex-1 flex-col">
            {!loading && userLimit && usersCount >= userLimit ?
              !platformSettings?.selfHosted ?
                <div className="border-accent/30 bg-accent/5 mb-6 flex flex-col gap-4 rounded-lg border p-5 sm:flex-row sm:items-center sm:justify-between">
                  <div className="flex items-start gap-4">
                    <div className="bg-accent/15 text-accent flex h-10 w-10 shrink-0 items-center justify-center rounded-full">
                      <Sparkles className="h-5 w-5" />
                    </div>
                    <div className="flex flex-col gap-1">
                      <p className="font-semibold">Bring your whole team on board</p>
                      <p className="text-content2 max-w-xl text-sm leading-6">
                        You're using all {userLimit} seat{userLimit > 1 ? 's' : ''} on your current plan. Add
                        teammates to collaborate on projects together — extra seats are billed at your plan's
                        per-user rate, and you can scale up or down anytime.
                      </p>
                    </div>
                  </div>
                  <div className="flex shrink-0 items-center gap-3 sm:pl-4">
                    <Button as="a" href={getRoutePath('workspaceBilling')} variant="accent">
                      Add seats
                    </Button>
                  </div>
                </div>
              : null
            : null}

            <MembersTable />
          </div>
        </PageContent>
      </WorkspaceSettingsLayout>
    </WorkspacesLayout>
  )
}
