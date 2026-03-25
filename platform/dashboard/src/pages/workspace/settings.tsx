import { yupResolver } from '@hookform/resolvers/yup'
import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { object, string } from 'yup'

import { Button } from '~/elements/Button'
import { TextField } from '~/elements/Input'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Setting, SettingsList } from '~/elements/Setting'
import { Skeleton } from '~/elements/Skeleton'
import { DeleteWorkspaceDialog } from '~/features/workspaces/components/DeleteWorkspaceDialog'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import { useCurrentWorkspaceQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { useUpdateWorkspaceMutation } from '~/features/workspaces/hooks/useWorkspaceMutations'
import { ConnectionsList } from '~/pages/workspace/connected-apps'

const workspaceNameSchema = object({
  name: string().min(1).max(256).required()
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
    resolver: yupResolver(workspaceNameSchema)
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
        await mutate({ id: workspace.id, ...values })
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
      <PageHeader contained>
        <PageTitle>Workspace Settings</PageTitle>
      </PageHeader>

      <PageContent contained>
        <SettingsList>
          <WorkspaceNameSetting />
          <DeleteWorkspaceSetting />
        </SettingsList>
        <div className="mt-8">
          <ConnectionsList />
        </div>
      </PageContent>
    </WorkspacesLayout>
  )
}
