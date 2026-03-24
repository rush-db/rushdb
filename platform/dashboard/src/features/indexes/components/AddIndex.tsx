import { useStore } from '@nanostores/react'

import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { Card, CardBody, CardFooter, CardHeader } from '~/elements/Card'
import { TextField } from '~/elements/Input'
import { object, string, useForm } from '~/lib/form'

import { addIndex } from '../stores/indexes'

const schema = object({
  propertyName: string().min(1)
})

export function AddIndexCard({ projectId }: { projectId: Project['id'] }) {
  const { error, mutate } = useStore(addIndex)

  const defaultValues = {
    propertyName: ''
  }

  const {
    formState: { errors, isSubmitting, isValid },
    handleSubmit,
    register,
    reset
  } = useForm({
    defaultValues,
    schema
  })

  return (
    <Card>
      <form
        onSubmit={handleSubmit((values) =>
          mutate({
            projectId,
            ...values
          }).then(() => reset(defaultValues))
        )}
      >
        <CardHeader title="Create embedding index" />
        <CardBody className="grid grid-cols-1 sm:grid-cols-3">
          <TextField
            {...register('propertyName')}
            error={errors.propertyName?.message}
            label="Property name"
            placeholder="description"
          />
        </CardBody>
        {error != null && (
          <div className="px-4 pb-2 text-sm text-red-500">
            {error instanceof Error ? error.message : 'Failed to create index'}
          </div>
        )}
        <CardFooter className="mt-5">
          <Button disabled={!isValid} loading={isSubmitting} type="submit" variant="accent">
            Create
          </Button>
        </CardFooter>
      </form>
    </Card>
  )
}
