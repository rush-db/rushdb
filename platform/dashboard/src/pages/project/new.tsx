import { useStore } from '@nanostores/react'

import { Button } from '~/elements/Button'
import { Card, CardBody, CardFooter, CardHeader } from '~/elements/Card'
import { TextField } from '~/elements/Input'
import { FormField } from '~/elements/FormField'
import { createProject } from '~/features/projects/stores/project'
import { ArrowRight, SparklesIcon } from 'lucide-react'
import { object, string, useForm } from '~/lib/form'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'
import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace'

import ArrowsGrid from '~/assets/icons/arrows-grid.svg'
import { $platformSettings } from '~/features/auth/stores/settings.ts'
import { setTourStep } from '~/features/tour/stores/tour.ts'
import { useEffect } from 'react'

// Type for form values
type ProjectFormValues = {
  name: string
  description?: string
  customDb?: {
    url: string
    username: string
    password: string
  }
}

// Schema for basic fields (free tier)
const baseSchema = object({
  description: string(),
  name: string().required().min(1).max(256)
}).required()

// Schema for premium fields (with customDb)
const premiumSchema = object({
  description: string(),
  name: string().required().min(1).max(256),
  customDb: object({
    url: string(),
    username: string(),
    password: string()
  })
}).required()

function CreateProjectForm({ className, ...props }: TPolymorphicComponentProps<'form'>) {
  const { error, mutate } = useStore(createProject)
  const { data: workspace } = useStore($currentWorkspace)
  const { loading, data: platformSettings } = useStore($platformSettings)

  // Check if user has a subscription
  const hasPaidPlan = workspace?.planId && workspace.planId !== 'free'
  const isSubscriptionActive = hasPaidPlan && !workspace?.isSubscriptionCancelled
  const hasValidSubscription =
    isSubscriptionActive ||
    (hasPaidPlan && workspace?.validTill && new Date(workspace.validTill) > new Date())

  useEffect(() => {
    setTourStep('newProjectName', false)
  }, [])

  // Use the appropriate schema based on subscription status
  const schema = hasValidSubscription ? premiumSchema : baseSchema

  const {
    formState: { errors, isSubmitted, isSubmitting },
    handleSubmit,
    register
  } = useForm<ProjectFormValues>({
    defaultValues: {
      description: '',
      name: '',
      ...(hasValidSubscription ? { customDb: { url: '', username: 'neo4j', password: '' } } : {})
    },
    schema
  })

  const success = !error && isSubmitted

  return (
    <Card className={cn('w-full', className)}>
      <form {...props} className={cn('flex flex-col gap-3')} onSubmit={handleSubmit(mutate)}>
        <CardHeader backHref={getRoutePath('projects')} title={'Create New Project'} />

        <CardBody>
          {loading || !platformSettings?.selfHosted ?
            <p className={'text-content-secondary text-sm'}>
              All projects come with a <strong>free shared instance</strong> with guaranteed data isolation
              and privacy. Your project can be created immediately without additional configuration.
            </p>
          : null}
          <TextField
            label="Name"
            {...register('name')}
            autoFocus
            error={errors?.name?.message}
            data-tour="project-name-input"
          />
          <TextField
            label="Description (optional)"
            {...register('description')}
            error={errors?.description?.message}
          />
        </CardBody>

        {loading || !platformSettings?.selfHosted ?
          <div className="relative" data-tour="custom-neo4j-container">
            {!hasValidSubscription ?
              <div className="blur-10 absolute top-0 z-0 h-full rounded-t-lg bg-gradient-to-b from-[#3f82ff] to-transparent to-50% opacity-20">
                <img src={ArrowsGrid} alt="Arrows Grid" className="" />
              </div>
            : null}
            <div className="relative z-10 p-5">
              <div className={cn({ 'mt-2': !hasValidSubscription })}>
                <h3 className="mb-2 px-1 font-semibold">Connect Your Own Neo4j Instance</h3>
                <div className="mb-4 space-y-3 px-1 text-sm">
                  <p className={'text-content-secondary text-sm'}>
                    For high-volume workloads and mission-critical applications requiring complete data
                    isolation, you can connect your own Neo4j database by upgrading your plan.
                  </p>
                </div>
                <div className="flex flex-col gap-4">
                  <FormField label="Database URL" error={errors?.customDb?.url?.message}>
                    <TextField
                      {...register('customDb.url')}
                      placeholder="bolt://your-database-url:7687"
                      disabled={!hasValidSubscription}
                    />
                  </FormField>
                  <FormField label="Username" error={errors?.customDb?.username?.message}>
                    <TextField
                      {...register('customDb.username')}
                      placeholder="neo4j"
                      disabled={!hasValidSubscription}
                    />
                  </FormField>
                  <FormField label="Password" error={errors?.customDb?.password?.message}>
                    <TextField
                      mb={2}
                      type="password"
                      {...register('customDb.password')}
                      placeholder="your-password"
                      disabled={!hasValidSubscription}
                    />
                  </FormField>
                </div>
              </div>
              {!hasValidSubscription && (
                <div className="mt-4 flex flex-col gap-3">
                  <Button
                    as="a"
                    href={getRoutePath('workspaceBilling')}
                    variant="accent"
                    className="flex w-full items-center justify-center"
                  >
                    <SparklesIcon />
                    Upgrade Plan
                  </Button>
                  <p className={'text-content-secondary text-sm'}>
                    <strong>Try all premium features free for 14 days</strong> - no payment required during
                    the trial period.
                  </p>
                </div>
              )}
            </div>
          </div>
        : null}

        <CardFooter>
          <Button loading={isSubmitting} type="submit" variant="accent" data-tour="create-project-btn">
            Create Project <ArrowRight className="mr-1 h-4 w-4" />
          </Button>
        </CardFooter>
      </form>
    </Card>
  )
}

export function NewProjectPage() {
  return (
    <div className="grid flex-1 place-items-center">
      <CreateProjectForm className="max-w-md" />
    </div>
  )
}
