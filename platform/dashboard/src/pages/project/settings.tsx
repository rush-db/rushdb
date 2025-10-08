import { yupResolver } from '@hookform/resolvers/yup'
import { useStore } from '@nanostores/react'
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
import { $currentProject } from '~/features/projects/stores/current-project'
import { updateProject } from '~/features/projects/stores/project'

const workspaceNameSchema = object({
  name: string().min(1).max(256).required()
})

function ProjectNameSetting({}: WithProjectID) {
  const { data: project } = useStore($currentProject)

  const { mutate } = useStore(updateProject)

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
      onSubmit={handleSubmit(mutate)}
      title="Project name"
    >
      <TextField {...register('name')} error={errors.name?.message} />
    </Setting>
  )
}

function DeleteProjectSetting({ projectId }: WithProjectID) {
  const { data: project, loading } = useStore($currentProject)

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
  const { data: project, loading } = useStore($currentProject)

  const statusDescription = (() => {
    if (!project) return null

    if (project.status === 'pending') {
      return `Subscription pending — please check your project subscription page to complete activation.`
    }

    if (project.status === 'provisioning') {
      const region = project.managedDbRegion || 'your selected region'
      return `Your project instance is currently provisioning in ${region}. Once provisioning finishes, your project will be ready to use.`
    }

    return null
  })()

  return (
    <>
      <PageHeader contained>
        <PageTitle>Project Settings</PageTitle>
      </PageHeader>
      <PageContent contained>
        <SettingsList>
          {statusDescription ?
            <div className="mb-4">
              <Message as="div" variant="info" size="medium" className={'mb-5 w-fit !p-4'}>
                <Skeleton enabled={loading}>{statusDescription}</Skeleton>
              </Message>
            </div>
          : null}

          <ProjectNameSetting projectId={projectId} />
          <DeleteProjectSetting projectId={projectId} />
        </SettingsList>
      </PageContent>
    </>
  )
}
