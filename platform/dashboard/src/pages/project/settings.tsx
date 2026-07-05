import { zodResolver } from '@hookform/resolvers/zod'
import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { z } from 'zod'

import type { WithProjectID } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { TextField } from '~/elements/Input'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Setting, SettingsList } from '~/elements/Setting'
import { Skeleton } from '~/elements/Skeleton'
import { DeleteProjectDialog } from '~/features/projects/components/DeleteProjectDialog'
import { useCurrentProjectQuery } from '~/features/projects/hooks/useProjectQueries'
import { useUpdateProjectMutation } from '~/features/projects/hooks/useProjectMutations'
import { ProjectSettingsLayout } from '~/features/projects/layout/ProjectSettingsLayout'
import { ConnectionsList } from '~/pages/workspace/connected-apps'

const workspaceNameSchema = z.object({
  name: z.string().min(1).max(256)
})

function ProjectNameSetting({}: WithProjectID) {
  const { data: project } = useCurrentProjectQuery()

  const { mutateAsync: mutate } = useUpdateProjectMutation()

  const {
    formState: { errors, isDirty, isSubmitting, isValid },
    handleSubmit,
    register,
    reset
  } = useForm({
    defaultValues: project,
    resolver: zodResolver(workspaceNameSchema)
  })

  useEffect(() => {
    reset(project)
  }, [reset, project])

  return (
    <Setting
      onReset={(event: { preventDefault: () => void }) => {
        event.preventDefault()
        reset()
      }}
      description="This is the name of your project"
      isDirty={isDirty}
      isSubmitting={isSubmitting}
      isValid={isValid}
      onSubmit={handleSubmit(async (values) => {
        if (!project?.id) return
        await mutate({ ...values, id: project.id })
      })}
      title="Project name"
    >
      <TextField {...register('name')} error={errors.name?.message} />
    </Setting>
  )
}

function DeleteProjectSetting({ projectId }: WithProjectID) {
  const { data: project, isPending: loading } = useCurrentProjectQuery()

  return (
    <Setting
      button={
        <DeleteProjectDialog
          trigger={
            <Button loading={loading} variant="danger">
              Delete project
            </Button>
          }
          projectId={projectId}
        />
      }
      title={
        <>
          Delete <Skeleton enabled={loading}>{project?.name ?? 'Loading'}</Skeleton>
        </>
      }
      className="border-danger"
      description={`Permanently delete your project and all of its contents. This action is not reversible, so please continue with caution.`}
    />
  )
}

export function ProjectSettings({ projectId }: WithProjectID) {
  return (
    <ProjectSettingsLayout projectId={projectId}>
      <PageHeader contained>
        <div className="flex max-w-3xl flex-col gap-2">
          <PageTitle>Project Settings</PageTitle>
          <p className="text-sm leading-6 text-content2">
            Manage the project name and deletion controls for this isolated data space.
          </p>
        </div>
      </PageHeader>
      <PageContent contained>
        <SettingsList>
          <ProjectNameSetting projectId={projectId} />
          <DeleteProjectSetting projectId={projectId} />
        </SettingsList>
      </PageContent>
    </ProjectSettingsLayout>
  )
}

export function ProjectConnectedApps({ projectId }: WithProjectID) {
  return (
    <ProjectSettingsLayout projectId={projectId}>
      <PageHeader contained>
        <div className="flex max-w-3xl flex-col gap-2">
          <PageTitle>Connected Apps</PageTitle>
          <p className="text-sm leading-6 text-content2">
            Review third-party applications authorized against this project. Revoking an app immediately
            invalidates its issued token.
          </p>
        </div>
      </PageHeader>
      <PageContent contained>
        <ConnectionsList
          emptyMessage="No connected applications have access to this project yet."
          projectId={projectId}
          showProject={false}
        />
      </PageContent>
    </ProjectSettingsLayout>
  )
}
