import { yupResolver } from '@hookform/resolvers/yup'
import { useStore } from '@nanostores/react'
import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { object, string } from 'yup'

import { Button } from '~/elements/Button'
import { TextField } from '~/elements/Input'
import { PageHeader, PageTitle } from '~/elements/PageHeader'
import { Setting, SettingsList } from '~/elements/Setting'
import { Skeleton } from '~/elements/Skeleton'
import { DeleteWorkspaceDialog } from '~/features/workspaces/components/DeleteWorkspaceDialog'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace'
import { updateWorkspace } from '~/features/workspaces/stores/mutations'

const workspaceNameSchema = object({
  name: string().min(1).max(256).required()
})

function WorkspaceNameSetting() {
  const { data: workspace } = useStore($currentWorkspace)

  const { mutate } = useStore(updateWorkspace)

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
      onReset={(event) => {
        event.preventDefault()
        reset()
      }}
      description="This will be a name of your workspace"
      isDirty={isDirty}
      isSubmitting={isSubmitting}
      isValid={isValid}
      onSubmit={handleSubmit(mutate)}
      title="Workspace name"
    >
      <TextField {...register('name')} error={errors.name?.message} />
    </Setting>
  )
}

function DeleteWorkspaceSetting() {
  const { data: workspace } = useStore($currentWorkspace)

  return (
    <Setting
      button={
        <DeleteWorkspaceDialog
          trigger={<Button variant="danger">Delete</Button>}
        />
      }
      title={
        <>
          Delete{' '}
          <Skeleton enabled={!workspace?.name}>
            {workspace?.name ?? 'Loading...'}
          </Skeleton>
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
      <PageHeader>
        <PageTitle>Workspace Settings</PageTitle>
      </PageHeader>
      <SettingsList>
        <WorkspaceNameSetting />
        <DeleteWorkspaceSetting />
      </SettingsList>
    </WorkspacesLayout>
  )
}
