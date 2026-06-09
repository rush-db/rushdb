import { Button } from '~/elements/Button'
import { Card, CardBody, CardFooter, CardHeader } from '~/elements/Card'
import { TextField } from '~/elements/Input'
import { useCreateWorkspaceMutation } from '~/features/workspaces/hooks/useWorkspaceMutations'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { object, string, useForm } from '~/lib/form'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'
import { useCallback, useEffect, useRef } from 'react'

const schema = object({
  name: string().required().min(1).max(256)
}).required()

function CreateWorkspaceForm({ ...props }: TPolymorphicComponentProps<'form'>) {
  const { data: newWorkspace, mutateAsync: mutate } = useCreateWorkspaceMutation()
  const nameInputRef = useRef<HTMLInputElement | null>(null)

  const {
    formState: { errors, isDirty, isSubmitted, isSubmitting },
    handleSubmit,
    register,
    setFocus,
    watch
  } = useForm({
    defaultValues: { name: '' },
    schema
  })

  const workspaceName = watch('name')
  const hasWorkspaceName = workspaceName.trim().length > 0
  const showSuccess = isDirty && newWorkspace !== undefined && isSubmitted
  const nameField = register('name')

  const focusNameInput = useCallback(() => {
    setFocus('name')
    nameInputRef.current?.focus({ preventScroll: true })
  }, [setFocus])

  useEffect(() => {
    const frameId = window.requestAnimationFrame(focusNameInput)
    const immediateId = window.setTimeout(focusNameInput, 0)
    const delayedId = window.setTimeout(focusNameInput, 100)

    return () => {
      window.cancelAnimationFrame(frameId)
      window.clearTimeout(immediateId)
      window.clearTimeout(delayedId)
    }
  }, [focusNameInput])

  return (
    <Card
      className={cn('w-full max-w-md', {
        'border-success': showSuccess
      })}
    >
      <form
        {...props}
        className={cn('flex flex-col gap-3')}
        onSubmit={handleSubmit(async (values) => mutate(values))}
      >
        <CardHeader
          backHref={getRoutePath('home')}
          title={showSuccess ? 'Success!' : 'Create New Workspace'}
        />

        <CardBody>
          <TextField
            label="Name"
            {...nameField}
            ref={(node) => {
              nameField.ref(node as unknown as HTMLInputElement | null)
              nameInputRef.current = node as unknown as HTMLInputElement | null
            }}
            autoFocus
            error={errors?.name?.message}
            readOnly={!!showSuccess}
          />
        </CardBody>

        <CardFooter>
          <Button
            disabled={showSuccess}
            loading={isSubmitting}
            type="submit"
            variant={hasWorkspaceName ? 'accent' : 'ghost'}
          >
            Create
          </Button>
        </CardFooter>
      </form>
    </Card>
  )
}

export function NewWorkspacePage() {
  const { data: platformSettings } = usePlatformSettings()

  if (!platformSettings?.selfHosted) {
    return (
      <div className="grid flex-1 place-items-center">
        <p className="text-content2">Creating additional workspaces is not available on cloud deployments.</p>
      </div>
    )
  }

  return (
    <div className="grid flex-1 place-items-center">
      <CreateWorkspaceForm />
    </div>
  )
}
