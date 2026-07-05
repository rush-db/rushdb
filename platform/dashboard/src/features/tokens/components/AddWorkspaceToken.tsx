import { useState } from 'react'
import { ArrowLeft } from 'lucide-react'

import type { ReactNode } from 'react'
import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { Checkbox } from '~/elements/Checkbox'
import { Dialog, DialogTitle } from '~/elements/Dialog'
import { CopyInput, TextField } from '~/elements/Input'
import { SelectField } from '~/elements/Select'
import { FormField } from '~/elements/FormField'
import { useForm, z } from '~/lib/form'

import type { ProjectToken } from '../types'

import { useAddWorkspaceTokenMutation } from '../hooks/useTokenMutations'

const schema = z.object({
  description: z.string().optional(),
  expiration: z.coerce.number().min(1).max(365).optional(),
  level: z.string().optional(),
  noExpire: z.boolean().optional(),
  name: z.string().min(3),
  projectId: z.string().min(1)
})

const levelOptions = [
  { label: 'Full access (read & write)', value: 'write' },
  { label: 'Read-only', value: 'read' }
]

function TokenCreated({ onBack, token }: { onBack: () => void; token: ProjectToken }) {
  return (
    <div className="mt-6 flex flex-col gap-5">
      <FormField caption={token.description} label={token.name}>
        <CopyInput value={token.value} />
      </FormField>
      <p className="text-sm text-content2">
        Copy this key now — for security reasons you won't be able to see it again.
      </p>
      {token.level === 'read' ?
        <p className="text-sm text-content2">
          This key is read-only: it can query data but never modify it, so it's safe to embed in client-side
          code and public demos.
        </p>
      : null}
      <div className="flex justify-end gap-2 border-t pt-5">
        <Button onClick={onBack} size="small" variant="secondary">
          <ArrowLeft />
          Create another
        </Button>
      </div>
    </div>
  )
}

function AddWorkspaceTokenForm({ projects }: { projects: Project[] }) {
  const { data: createdToken, error, mutateAsync: mutate } = useAddWorkspaceTokenMutation()

  const defaultValues = {
    description: '',
    expiration: 30,
    level: 'write',
    noExpire: false,
    name: 'API key',
    projectId: projects[0]?.id ?? ''
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
          ...values,
          expiration: `${values.expiration}d`,
          level: values.level as ProjectToken['level']
        })
      )}
    >
      <SelectField
        {...register('projectId')}
        error={errors.projectId?.message}
        label="Project"
        options={projects.map((project) => ({ label: project.name, value: project.id }))}
      />
      <div className="grid grid-cols-1 gap-5 sm:grid-cols-2">
        <TextField {...register('name')} error={errors.name?.message} label="API key name" />
        <TextField
          {...register('description')}
          error={errors.description?.message}
          label="API key description"
        />
      </div>
      <div>
        <SelectField
          {...register('level')}
          className="mb-2"
          error={errors.level?.message}
          label="Permissions"
          options={levelOptions}
        />
        <p className="text-sm text-content2">
          Read-only keys can only query data — safe to embed in client-side code and public demos. Full access
          keys must stay server-side.
        </p>
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

export function AddWorkspaceTokenDialog({ projects, trigger }: { projects: Project[]; trigger: ReactNode }) {
  const [open, setOpen] = useState(false)

  return (
    <Dialog className="sm:max-w-2xl" onOpenChange={setOpen} open={open} trigger={trigger}>
      <DialogTitle>Create API key</DialogTitle>
      <p className="mt-2 text-content2">
        API keys authenticate requests from the SDKs and REST API. Choose the project this key grants access
        to, give it a name, and optionally an expiration.
      </p>
      <AddWorkspaceTokenForm projects={projects} />
    </Dialog>
  )
}
