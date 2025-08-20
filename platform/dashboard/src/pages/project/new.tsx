import { useStore } from '@nanostores/react'

import { Button } from '~/elements/Button'
import { PageContent, PageHeader } from '~/elements/PageHeader'
import { TextField } from '~/elements/Input'
import { FormField } from '~/elements/FormField'
import { createProject } from '~/features/projects/stores/project'
import { ArrowRight, SparklesIcon, Database, Cloud, Globe } from 'lucide-react'
import { object, string, useForm } from '~/lib/form'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'
import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace'

import { $platformSettings } from '~/features/auth/stores/settings.ts'
import { setTourStep } from '~/features/tour/stores/tour.ts'
import { useEffect, useState } from 'react'
import { useWatch } from 'react-hook-form'
import { Label } from '~/elements/Label'
import { $showUpgrade } from '~/features/workspaces/stores/projects'
import { AWS_REGIONS } from '~/features/projects/constants.ts'

// Type for form values
type ProjectFormValues = {
  name: string
  description?: string
  dataSource: 'shared' | 'custom' | 'managed'
  customDb?: {
    url: string
    username: string
    password: string
  }
  managedDb?: {
    password: string
    region: string
  }
}

// Password strength validation
const isStrongPassword = (password: string) => {
  return (
    password.length >= 8 &&
    /[A-Z]/.test(password) &&
    /[a-z]/.test(password) &&
    /[0-9]/.test(password) &&
    /[^A-Za-z0-9]/.test(password) &&
    // no whitespace allowed
    !/\s/.test(password)
  )
}

// Schema for shared db (free tier)
const sharedSchema = object({
  description: string(),
  name: string().required().min(1).max(256),
  dataSource: string().required()
}).required()

// Schema for custom db
const customSchema = object({
  description: string(),
  name: string().required().min(1).max(256),
  dataSource: string().required(),
  customDb: object({
    url: string().required(),
    username: string().required(),
    password: string().required()
  }).required()
}).required()

// Schema for managed db
const managedSchema = object({
  description: string(),
  name: string().required().min(1).max(256),
  dataSource: string().required(),
  managedDb: object({
    password: string()
      .required()
      .test(
        'strong',
        'Password must be at least 8 characters with uppercase, lowercase, number, special character, and cannot contain spaces',
        isStrongPassword
      ),
    region: string().required()
  }).required()
}).required()

function CreateProjectForm({ className, ...props }: TPolymorphicComponentProps<'form'>) {
  const { error, mutate } = useStore(createProject)
  const { data: workspace } = useStore($currentWorkspace)
  const { loading, data: platformSettings } = useStore($platformSettings)
  const showUpgradeButton = useStore($showUpgrade)

  const [selectedTab, setSelectedTab] = useState<'shared' | 'custom' | 'managed'>('shared')

  // Check if user has a subscription
  const hasPaidPlan = workspace?.planId && workspace.planId !== 'free'
  const isSubscriptionActive = hasPaidPlan && !workspace?.isSubscriptionCancelled
  const hasValidSubscription =
    isSubscriptionActive ||
    (hasPaidPlan && workspace?.validTill && new Date(workspace.validTill) > new Date())

  useEffect(() => {
    setTourStep('newProjectName', false)
  }, [])

  // Use the appropriate schema based on selected tab
  const getSchema = () => {
    switch (selectedTab) {
      case 'shared':
        return sharedSchema
      case 'custom':
        return customSchema
      case 'managed':
        return managedSchema
      default:
        return sharedSchema
    }
  }

  const getDefaultValues = (): Partial<ProjectFormValues> => {
    const base = {
      description: '',
      name: '',
      dataSource: selectedTab as ProjectFormValues['dataSource']
    }

    switch (selectedTab) {
      case 'custom':
        return { ...base, customDb: { url: '', username: 'neo4j', password: '' } }
      case 'managed':
        return { ...base, managedDb: { password: '', region: 'us-east-1' } }
      default:
        return base
    }
  }

  const {
    formState: { errors, isSubmitted, isSubmitting },
    handleSubmit,
    register,
    watch,
    reset,
    control,
    setValue
  } = useForm<ProjectFormValues>({
    defaultValues: getDefaultValues(),
    schema: getSchema()
  })

  // Reset form when tab changes
  useEffect(() => {
    reset(getDefaultValues())
    // keep form dataSource in sync for schema
    setValue('dataSource', selectedTab)
  }, [selectedTab, reset, setValue])

  // Subscribe to managed password changes for real-time strength updates
  const watchedPassword = useWatch({ control, name: 'managedDb.password' })
  const passwordStrength = watchedPassword ? isStrongPassword(watchedPassword) : false

  const handleFormSubmit = (data: ProjectFormValues) => {
    // Block custom DB submission without valid subscription
    if (selectedTab === 'custom' && !hasValidSubscription) {
      return
    }
    // Transform data to match the expected Project type
    const projectData = {
      name: data.name,
      description: data.description,
      ...(data.customDb && { customDb: data.customDb }),
      ...(data.managedDb && {
        managedDbConfig: {
          password: data.managedDb.password,
          region: data.managedDb.region,
          // Tier will be selected in the next step; using placeholder
          tier: 'unselected'
        }
      })
    }
    // @TODO: Sanitize project payload
    // @ts-expect-error customDb and stats are strings on the backend
    return mutate(projectData)
  }

  const success = !error && isSubmitted

  return (
    <>
      <PageContent contained>
        <div className="mx-auto w-full max-w-2xl">
          <div className="mb-8 flex items-center gap-3 pt-10">
            <h1 className="text-2xl font-bold">Create New Project</h1>
          </div>
          <form
            {...props}
            className={cn('flex flex-col gap-6', className)}
            onSubmit={handleSubmit(handleFormSubmit)}
            autoComplete="off"
          >
            {/* Project Type Selection (Radio-style tabs) */}
            <div className="space-y-2">
              <p className="text-content-secondary text-sm font-medium tracking-wide">Type</p>
              <div className="grid gap-3 md:grid-cols-1">
                {/* Shared */}
                <button
                  type="button"
                  onClick={() => setSelectedTab('shared')}
                  className={cn(
                    'group relative flex h-full flex-col items-start rounded-lg border p-4 text-left transition-all',
                    'focus:ring-accent/60 focus:outline-none focus:ring-2',
                    selectedTab === 'shared' ?
                      'border-accent/60 ring-accent/60 bg-accent/5 ring-1'
                    : 'border-border hover:border-accent/40 hover:bg-surface-secondary'
                  )}
                >
                  <div className="mb-3 flex w-full items-center justify-between">
                    <div className="flex items-center gap-2">
                      <div
                        className={cn(
                          'h-4 w-4 rounded-full border',
                          selectedTab === 'shared' ? 'border-accent bg-accent shadow-inner' : 'border-border'
                        )}
                      />
                      <Database className="h-4 w-4" />
                    </div>
                  </div>
                  <h3 className="mb-1 text-lg font-semibold leading-snug">
                    Basic Features{' '}
                    <span className="text-content2 font-normal">[comes with Shared Instance]</span>
                  </h3>
                  <p className="text-content2 text-sm">
                    For testing ideas and small projects. Limited in using custom queries.
                  </p>
                </button>

                {/* Extended / Custom */}
                <button
                  type="button"
                  onClick={() => setSelectedTab('custom')}
                  className={cn(
                    'group relative flex h-full flex-col items-start rounded-lg border p-4 text-left transition-all',
                    'focus:ring-accent/60 focus:outline-none focus:ring-2',
                    selectedTab === 'custom' ?
                      'border-accent/60 ring-accent/60 bg-accent/5 ring-1'
                    : 'border-border hover:border-accent/40 hover:bg-surface-secondary'
                  )}
                >
                  <div className="mb-3 flex w-full items-center justify-between">
                    <div className="flex items-center gap-2">
                      <div
                        className={cn(
                          'h-4 w-4 rounded-full border',
                          selectedTab === 'custom' ? 'border-accent bg-accent shadow-inner' : 'border-border'
                        )}
                      />
                      <Globe className="h-4 w-4" />
                    </div>
                    <div className="flex items-center gap-1">
                      <Label>PRO</Label>
                    </div>
                  </div>
                  <h3 className="mb-1 text-lg font-semibold leading-snug">
                    Extended Features{' '}
                    <span className="text-content2 font-normal">[Shared or Bring your own cloud]</span>
                  </h3>
                  <p className="text-content2 text-sm">
                    Higher limits on shared infra or bring your own Neo4j (Aura or hosted). No upper limits.
                    Custom queries.
                  </p>
                </button>

                {/* Managed */}
                <button
                  type="button"
                  onClick={() => setSelectedTab('managed')}
                  className={cn(
                    'group relative flex h-full flex-col items-start rounded-lg border p-4 text-left transition-all',
                    'focus:ring-accent/60 focus:outline-none focus:ring-2',
                    selectedTab === 'managed' ?
                      'border-accent/60 ring-accent/60 bg-accent/5 ring-1'
                    : 'border-border hover:border-accent/40 hover:bg-surface-secondary'
                  )}
                >
                  <div className="mb-3 flex w-full items-center justify-between">
                    <div className="flex items-center gap-2">
                      <div
                        className={cn(
                          'h-4 w-4 rounded-full border',
                          selectedTab === 'managed' ? 'border-accent bg-accent shadow-inner' : 'border-border'
                        )}
                      />
                      <Cloud className="h-4 w-4" />
                    </div>
                    <div className="flex items-center gap-1">
                      <Label>TEAM</Label>{' '}
                    </div>
                  </div>
                  <h3 className="mb-1 text-lg font-semibold leading-snug">
                    Fully Featured <span className="text-content2 font-normal">[Dedicated instance]</span>
                  </h3>
                  <p className="text-content2 text-sm">
                    Dedicated instance, highest security standards, extended team features, custom queries. No
                    usage limits. Scale on demand.
                  </p>
                </button>
              </div>
            </div>

            {/* hidden form bridge */}
            <input type="hidden" value={selectedTab} {...register('dataSource')} />

            {/* Basic Fields */}
            <div className="grid gap-4">
              <TextField
                label="Name"
                {...register('name')}
                autoFocus
                error={errors?.name?.message}
                data-tour="project-name-input"
                autoComplete="off"
                disabled={(selectedTab === 'shared' && showUpgradeButton) || isSubmitting}
              />
              <TextField
                label="Description (optional)"
                {...register('description')}
                error={errors?.description?.message}
                autoComplete="off"
                disabled={(selectedTab === 'shared' && showUpgradeButton) || isSubmitting}
              />
            </div>

            {selectedTab === 'shared' && showUpgradeButton && (
              <div className="bg-surface-secondary border-accent/20 rounded-lg border p-4 text-sm">
                <p className="text-content2 mb-3">
                  You've reached the maximum number of projects for your current plan.
                </p>
                <Button
                  as="a"
                  href={getRoutePath('workspaceBilling')}
                  variant="accent"
                  className="w-full justify-center"
                >
                  <SparklesIcon className="h-4 w-4" /> Upgrade Plan
                </Button>
              </div>
            )}

            <div className="">
              {selectedTab === 'custom' && (
                <>
                  <div className="mt-4">
                    <div className="bg-surface-secondary border-accent/20 mb-4 rounded-lg border p-4">
                      <h3 className="mb-2 font-semibold">Bring Your Own Neo4j Instance</h3>
                      <p className="text-content-secondary mb-3 text-sm">
                        Connect your own Neo4j instance (Aura or self-hosted) for unlimited scalability and
                        full control over your data.
                      </p>
                      {!hasValidSubscription && (
                        <div className="border-border mt-3 border-t pt-3">
                          <Button
                            as="a"
                            href={getRoutePath('workspaceBilling')}
                            variant="accent"
                            className="w-full justify-center"
                          >
                            <SparklesIcon className="h-4 w-4" />
                            Upgrade to Pro
                          </Button>
                        </div>
                      )}
                    </div>

                    <div>
                      <FormField label="Database URL" error={errors?.customDb?.url?.message}>
                        <TextField
                          {...register('customDb.url')}
                          placeholder="bolt://your-database-url:7687"
                          autoComplete="off"
                          inputMode="url"
                          disabled={!hasValidSubscription}
                        />
                      </FormField>
                      <FormField label="Username" error={errors?.customDb?.username?.message}>
                        <TextField
                          {...register('customDb.username')}
                          placeholder="neo4j"
                          autoComplete="off"
                          disabled={!hasValidSubscription}
                        />
                      </FormField>
                      <FormField label="Password" error={errors?.customDb?.password?.message}>
                        <TextField
                          type="password"
                          {...register('customDb.password')}
                          placeholder="your-password"
                          autoComplete="new-password"
                          spellCheck={false}
                          disabled={!hasValidSubscription}
                        />
                      </FormField>
                    </div>
                  </div>
                </>
              )}

              {selectedTab === 'managed' && (
                <div>
                  <div className="space-y-4">
                    <FormField label="Instance Password" error={errors?.managedDb?.password?.message}>
                      <div className="space-y-2">
                        <TextField
                          type="password"
                          {...register('managedDb.password')}
                          placeholder="Create a strong password"
                          autoComplete="new-password"
                          spellCheck={false}
                        />
                        <p className="text-content-secondary text-content2 text-xs">
                          Must contain at least 8 characters with uppercase, lowercase, number, and special
                          character. Spaces are not allowed.
                        </p>
                        {watchedPassword && (
                          <div className="flex items-center gap-2">
                            <div
                              className={cn(
                                'bg-surface-secondary h-2 w-full rounded-full',
                                passwordStrength ? 'bg-green-200' : 'bg-red-200'
                              )}
                            >
                              <div
                                className={cn(
                                  'h-full rounded-full transition-all',
                                  passwordStrength ? 'w-full bg-green-500' : 'w-1/3 bg-red-500'
                                )}
                              />
                            </div>
                            <span
                              className={cn('text-xs', passwordStrength ? 'text-green-600' : 'text-red-600')}
                            >
                              {passwordStrength ? 'Strong' : 'Weak'}
                            </span>
                          </div>
                        )}
                      </div>
                    </FormField>

                    <FormField label="Region" error={errors?.managedDb?.region?.message}>
                      <select
                        {...register('managedDb.region')}
                        className="border-border bg-surface text-content focus:ring-accent focus:border-accent w-full rounded-md border px-3 py-2 focus:outline-none focus:ring-2"
                        autoComplete="off"
                      >
                        {AWS_REGIONS.map((region) => (
                          <option key={region.code} value={region.code}>
                            {region.flag} {region.name}, {region.country}
                          </option>
                        ))}
                      </select>
                    </FormField>
                  </div>
                </div>
              )}
            </div>

            <div className="flex items-center justify-between gap-4">
              {selectedTab === 'managed' ?
                <span className="text-content2 text-sm">
                  Managed instances are billed per project. Create the project first; on the next step you'll
                  choose a tier and complete payment.
                </span>
              : <div />}
              <Button
                loading={isSubmitting}
                type="submit"
                variant="accent"
                data-tour="create-project-btn"
                disabled={
                  (selectedTab === 'custom' && !hasValidSubscription) ||
                  (selectedTab === 'shared' && showUpgradeButton)
                }
                title={
                  selectedTab === 'custom' && !hasValidSubscription ?
                    'Upgrade to connect your own Neo4j instance'
                  : selectedTab === 'shared' && showUpgradeButton ?
                    'Upgrade plan to create more projects'
                  : undefined
                }
              >
                Create Project <ArrowRight className="mr-1 h-4 w-4" />
              </Button>
            </div>
          </form>
        </div>
      </PageContent>
    </>
  )
}

export function NewProjectPage() {
  return (
    <div className="flex flex-1 flex-col">
      <CreateProjectForm />
    </div>
  )
}
