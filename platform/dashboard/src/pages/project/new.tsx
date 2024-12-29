import { useStore } from '@nanostores/react'

import { Button } from '~/elements/Button'
import { Card, CardBody, CardFooter, CardHeader } from '~/elements/Card'
import { TextField } from '~/elements/Input'
import { createProject } from '~/features/projects/stores/project'
import { object, string, useForm } from '~/lib/form'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'

const schema = object({
  description: string(),
  name: string().required().min(1).max(256)
}).required()

function CreateProjectForm({
  className,
  ...props
}: TPolymorphicComponentProps<'form'>) {
  const { error, mutate } = useStore(createProject)

  const {
    formState: { errors, isSubmitted, isSubmitting },
    handleSubmit,
    register
  } = useForm({
    defaultValues: { description: '', name: '' },
    schema
  })

  const success = !error && isSubmitted

  return (
    <Card className={cn('w-full', className)}>
      <form
        {...props}
        className={cn('flex flex-col gap-3')}
        onSubmit={handleSubmit(mutate)}
      >
        <CardHeader
          backHref={getRoutePath('projects')}
          title={'Start New Project'}
        />

        <CardBody>
          <TextField
            label="Name"
            {...register('name')}
            autoFocus
            error={errors?.name?.message}
          />
          <TextField
            label="Description (optional)"
            {...register('description')}
            error={errors?.description?.message}
          />
        </CardBody>

        <CardFooter>
          <Button loading={isSubmitting} type="submit" variant="accent">
            Create
          </Button>
        </CardFooter>
      </form>
    </Card>
  )
}

export function NewProjectPage() {
  return (
    <div className="grid flex-1 place-items-center ">
      <CreateProjectForm className="max-w-md" />
    </div>
  )
}
