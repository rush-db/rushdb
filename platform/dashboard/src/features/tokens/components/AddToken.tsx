import { useState } from 'react'
import { ArrowLeft } from 'lucide-react'

import type { ReactNode } from 'react'
import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { Checkbox } from '~/elements/Checkbox'
import { Dialog, DialogTitle } from '~/elements/Dialog'
import { CopyInput, TextField } from '~/elements/Input'
import { FormField } from '~/elements/FormField'
import { boolean, number, object, string, useForm } from '~/lib/form'

import type { ProjectToken } from '../types'

import { useAddTokenMutation } from '../hooks/useTokenMutations'

const schema = object({
  description: string(),
  expiration: number().min(1).max(365).optional(),
  noExpire: boolean().optional(),
  name: string().min(3)
})

function TokenCreated({ onBack, token }: { onBack: () => void; token: ProjectToken }) {
  return (
    <div className="mt-6 flex flex-col gap-5">
      <FormField caption={token.description} label={token.name}>
        <CopyInput value={token.value} />
      </FormField>
      <p className="text-content2 text-sm">
        Copy this key now — for security reasons you won't be able to see it again.
      </p>
      <div className="flex justify-end gap-2 border-t pt-5">
        <Button onClick={onBack} size="small" variant="secondary">
          <ArrowLeft />
          Create another
        </Button>
      </div>
    </div>
  )
}

function AddTokenForm({ project, projectId }: { project?: Project; projectId: Project['id'] }) {
  const { data: createdToken, error, mutateAsync: mutate } = useAddTokenMutation()

  const defaultValues = {
    description: '',
    expiration: 30,
    noExpire: false,
    name: `${project?.name ?? ''} token`
  }

  const {
    formState: { errors, isSubmitted, isSubmitting, isValid },
    handleSubmit,
    register,
    reset,
    setValue,
    watch
  } = useForm({
    defaultValues,
    schema
  })
  const noExpire = watch('noExpire', false)

  const showSuccess = createdToken && !error && isSubmitted

  if (showSuccess) {
    return <TokenCreated onBack={() => reset(defaultValues)} token={createdToken} />
  }

  return (
    <form
      className="mt-6 flex flex-col gap-5"
      onSubmit={handleSubmit((values) =>
        mutate({
          projectId,
          ...values,
          expiration: `${values.expiration}d`,
          level: 'write'
        })
      )}
    >
      <div className="grid grid-cols-1 gap-5 sm:grid-cols-2">
        <TextField {...register('name')} error={errors.name?.message} label="API key name" />
        <TextField
          {...register('description')}
          error={errors.description?.message}
          label="API key description"
        />
      </div>
      <div>
        <TextField
          {...register('expiration')}
          className="mb-3"
          disabled={noExpire}
          error={errors.expiration?.message}
          label="Duration, in days"
          type="number"
        />
        <label className="flex w-fit cursor-pointer items-center gap-2 text-sm">
          <Checkbox
            checked={noExpire}
            onCheckedChange={(checked) => setValue('noExpire', checked === true, { shouldValidate: true })}
          />
          <span className="select-none">No expiration</span>
        </label>
      </div>
      <div className="flex justify-end gap-2 border-t pt-5">
        <Button disabled={!isValid} loading={isSubmitting} type="submit" variant="primary">
          Create API key
        </Button>
      </div>
    </form>
  )
}

export function AddTokenDialog({
  project,
  projectId,
  trigger
}: {
  project?: Project
  projectId: Project['id']
  trigger: ReactNode
}) {
  const [open, setOpen] = useState(false)

  return (
    <Dialog className="sm:max-w-2xl" onOpenChange={setOpen} open={open} trigger={trigger}>
      <DialogTitle>Create API key</DialogTitle>
      <p className="text-content2 mt-2">
        API keys authenticate requests to this project from the SDKs and REST API. Give the key a name, and
        optionally an expiration.
      </p>
      <AddTokenForm project={project} projectId={projectId} />
    </Dialog>
  )
}
