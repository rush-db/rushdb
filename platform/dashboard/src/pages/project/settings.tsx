import { yupResolver } from '@hookform/resolvers/yup'
import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { object, string } from 'yup'

import type { WithProjectID } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { TextField } from '~/elements/Input'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Setting, SettingsList } from '~/elements/Setting'
import { Skeleton } from '~/elements/Skeleton'
import { Message } from '~/elements/Message'
import { DeleteProjectDialog } from '~/features/projects/components/DeleteProjectDialog'
import { useCurrentProjectQuery } from '~/features/projects/hooks/useProjectQueries'
import { useUpdateProjectMutation } from '~/features/projects/hooks/useProjectMutations'

const workspaceNameSchema = object({
  name: string().min(1).max(256).required()
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
    resolver: yupResolver(workspaceNameSchema)
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
        await mutate({ id: project.id, ...values })
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
  const { data: project, isPending: loading } = useCurrentProjectQuery()

  return (
    <>
      <PageHeader contained>
        <PageTitle>Project Settings</PageTitle>
      </PageHeader>
      <PageContent contained>
        <SettingsList>
          <ProjectNameSetting projectId={projectId} />
          <DeleteProjectSetting projectId={projectId} />
        </SettingsList>
      </PageContent>
    </>
  )
}
