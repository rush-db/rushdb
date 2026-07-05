import { zodResolver } from '@hookform/resolvers/zod'
import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { z } from 'zod'

import { Button } from '~/elements/Button'
import { TextField } from '~/elements/Input'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Setting, SettingsList } from '~/elements/Setting'
import { Skeleton } from '~/elements/Skeleton'
import { DeleteWorkspaceDialog } from '~/features/workspaces/components/DeleteWorkspaceDialog'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import { WorkspaceSettingsLayout } from '~/features/workspaces/layout/WorkspaceSettingsLayout'
import { useCurrentWorkspaceQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { useUpdateWorkspaceMutation } from '~/features/workspaces/hooks/useWorkspaceMutations'

const workspaceNameSchema = z.object({
  name: z.string().min(1).max(256)
})

function WorkspaceNameSetting() {
  const { data: workspace } = useCurrentWorkspaceQuery()

  const { mutateAsync: mutate } = useUpdateWorkspaceMutation()

  const {
    formState: { errors, isDirty, isSubmitting, isValid },
    handleSubmit,
    register,
    reset
  } = useForm({
    defaultValues: workspace,
    resolver: zodResolver(workspaceNameSchema)
  })

  useEffect(() => {
    reset(workspace)
  }, [reset, workspace])

  return (
    <Setting
      onReset={(event: { preventDefault: () => void }) => {
        event.preventDefault()
        reset()
      }}
      description="This will be a name of your workspace"
      isDirty={isDirty}
      isSubmitting={isSubmitting}
      isValid={isValid}
      onSubmit={handleSubmit(async (values) => {
        if (!workspace?.id) return
        await mutate({ ...values, id: workspace.id })
      })}
      title="Workspace name"
    >
      <TextField {...register('name')} error={errors.name?.message} />
    </Setting>
  )
}

function DeleteWorkspaceSetting() {
  const { data: workspace } = useCurrentWorkspaceQuery()

  return (
    <Setting
      button={<DeleteWorkspaceDialog trigger={<Button variant="danger">Delete</Button>} />}
      title={
        <>
          Delete <Skeleton enabled={!workspace?.name}>{workspace?.name ?? 'Loading...'}</Skeleton>
        </>
      }
      className="border-danger"
      description={`Permanently delete your workspace and all of its contents. This action is not reversible, so please continue with caution.`}
    />
  )
}

export function WorkspaceSettingsPage() {
  return (
    <WorkspacesLayout>
      <WorkspaceSettingsLayout>
        <PageHeader contained>
          <div className="flex max-w-3xl flex-col gap-2">
            <PageTitle>General</PageTitle>
            <p className="text-sm leading-6 text-content2">
              Configure workspace identity and lifecycle controls. These settings apply to the workspace
              container, not to an individual project.
            </p>
          </div>
        </PageHeader>

        <PageContent contained>
          <SettingsList>
            <WorkspaceNameSetting />
          </SettingsList>
          <SettingsList>
            <DeleteWorkspaceSetting />
          </SettingsList>
        </PageContent>
      </WorkspaceSettingsLayout>
    </WorkspacesLayout>
  )
}
