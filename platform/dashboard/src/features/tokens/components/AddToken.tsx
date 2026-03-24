import { ArrowLeft } from 'lucide-react'

import type { Project } from '~/features/projects/types'

import { Button, CopyButton } from '~/elements/Button'
import { Card, CardBody, CardFooter, CardHeader } from '~/elements/Card'
import { CopyInput, TextField } from '~/elements/Input'
import { boolean, number, object, string, useForm } from '~/lib/form'

import type { ProjectToken } from '../types'

import { useAddTokenMutation } from '../hooks/useTokenMutations'
import { FormField } from '~/elements/FormField'

const schema = object({
  description: string(),
  expiration: number().min(1).max(365).optional(),
  noExpire: boolean().optional(),
  name: string().min(3),
  level: string().optional()
})

function TokenCreated({
  onBack,
  token
}: // setOpen
{
  onBack: () => void
  token: ProjectToken
  // setOpen: (open: boolean) => void
}) {
  return (
    <>
      <CardHeader title="API key successfully created" />
      <CardBody>
        <FormField label={token.name} caption={token.description}>
          <CopyInput value={token.value} />
        </FormField>
      </CardBody>
      <CardFooter className="justify-between">
        <Button onClick={onBack} variant="secondary" size="small">
          <ArrowLeft />
          New token
        </Button>
      </CardFooter>
    </>
  )
}

export function AddTokenCard({ projectId, project }: { project?: Project; projectId: Project['id'] }) {
  const { data: createdToken, error, mutateAsync: mutate } = useAddTokenMutation()

  const defaultValues = {
    description: '',
    expiration: 30,
    noExpire: false,
    name: `${project?.name ?? ''} token`,
    level: 'write' as 'read' | 'write'
  }

  const {
    formState: { errors, isSubmitted, isSubmitting, isValid },
    handleSubmit,
    register,
    reset,
    watch
  } = useForm({
    defaultValues,
    schema
  })
  // @ts-ignore
  const expirationDisabled = watch('noExpire', false)
  // useEffect(() => {
  //   reset(defaultValues)
  // }, [reset, project])
  const showSuccess = createdToken && !error && isSubmitted

  return (
    <Card>
      {showSuccess ?
        <TokenCreated onBack={() => reset(defaultValues)} token={createdToken} />
      : <form
          onSubmit={handleSubmit((values) =>
            mutate({
              projectId,
              ...values,
              expiration: `${values.expiration}d`,
              level: values.level ?? 'write'
            })
          )}
        >
          <CardHeader title="Create token" />
          <CardBody className="grid grid-cols-1 sm:grid-cols-3">
            <TextField {...register('name')} error={errors.name?.message} label="API key name" />
            <TextField
              {...register('description')}
              error={errors.description?.message}
              label="API key description"
            />
            <FormField label="Access level">
              <div className="flex flex-col gap-2 pt-1">
                <label className="flex cursor-pointer items-center gap-2 text-sm">
                  <input type="radio" value="write" {...register('level')} defaultChecked />
                  <span className="font-medium">Read &amp; Write</span>
                  <span className="text-content-2 text-xs">Full access</span>
                </label>
                <label className="flex cursor-pointer items-center gap-2 text-sm">
                  <input type="radio" value="read" {...register('level')} />
                  <span className="font-medium">Read Only</span>
                  <span className="text-content-2 text-xs">No mutations</span>
                </label>
              </div>
            </FormField>
            <div className="">
              <TextField
                {...register('expiration')}
                className="mb-2"
                disabled={expirationDisabled}
                error={errors.expiration?.message}
                label="Duration, in days"
                type="number"
              />
              <label>
                <input type="checkbox" defaultChecked {...register('noExpire')} />
                <span className="ml-2 select-none text-start text-sm">No expiration</span>
              </label>
            </div>
          </CardBody>
          <CardFooter className="mt-5">
            <Button disabled={!isValid} loading={isSubmitting} type="submit" variant="accent">
              Create
            </Button>
          </CardFooter>
        </form>
      }
    </Card>
  )
}
