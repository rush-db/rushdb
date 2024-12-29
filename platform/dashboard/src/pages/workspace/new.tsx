import { useStore } from '@nanostores/react'

import { Button } from '~/elements/Button'
import { Card, CardBody, CardFooter, CardHeader } from '~/elements/Card'
import { TextField } from '~/elements/Input'
import { createWorkspace } from '~/features/workspaces/stores/mutations'
import { object, string, useForm } from '~/lib/form'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'

const schema = object({
  name: string().required().min(1).max(256)
}).required()

function CreateWorkspaceForm({ ...props }: TPolymorphicComponentProps<'form'>) {
  const { data: newWorkspace, mutate } = useStore(createWorkspace)

  const {
    formState: { errors, isDirty, isSubmitted, isSubmitting },
    handleSubmit,
    register
  } = useForm({
    defaultValues: { name: '' },
    schema
  })

  const showSuccess = isDirty && newWorkspace !== undefined && isSubmitted

  return (
    <Card
      className={cn('w-full max-w-md ', {
        'border-success': showSuccess
      })}
    >
      <form
        {...props}
        className={cn('flex flex-col gap-3')}
        onSubmit={handleSubmit(mutate)}
      >
        <CardHeader
          backHref={getRoutePath('home')}
          title={showSuccess ? 'Success!' : 'Create New Workspace'}
        />

        <CardBody>
          <TextField
            label="Name"
            {...register('name')}
            autoFocus
            error={errors?.name?.message}
            readOnly={!!showSuccess}
          />
        </CardBody>

        <CardFooter>
          <Button disabled={showSuccess} loading={isSubmitting} type="submit">
            Create
          </Button>
        </CardFooter>
      </form>
    </Card>
  )
}

export function NewWorkspacePage() {
  return (
    <div className="grid flex-1 place-items-center">
      <CreateWorkspaceForm />
    </div>
  )
}
