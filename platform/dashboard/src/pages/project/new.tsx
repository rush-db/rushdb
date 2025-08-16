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
    /[^A-Za-z0-9]/.test(password)
  )
}

// Available AWS regions
const AWS_REGIONS = [
  { code: 'us-east-1', name: 'N. Virginia', country: 'US', flag: '🇺🇸' },
  { code: 'us-west-2', name: 'Oregon', country: 'US', flag: '🇺🇸' },
  { code: 'eu-west-1', name: 'Ireland', country: 'IE', flag: '🇮🇪' },
  { code: 'eu-central-1', name: 'Frankfurt', country: 'DE', flag: '🇩🇪' },
  { code: 'ap-southeast-1', name: 'Singapore', country: 'SG', flag: '🇸🇬' },
  { code: 'ap-northeast-1', name: 'Tokyo', country: 'JP', flag: '🇯🇵' }
]

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
        'Password must be at least 8 characters with uppercase, lowercase, number, and special character',
        isStrongPassword
      ),
    region: string().required()
  }).required()
}).required()

function CreateProjectForm({ className, ...props }: TPolymorphicComponentProps<'form'>) {
  const { error, mutate } = useStore(createProject)
  const { data: workspace } = useStore($currentWorkspace)
  const { loading, data: platformSettings } = useStore($platformSettings)

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
    control
  } = useForm<ProjectFormValues>({
    defaultValues: getDefaultValues(),
    schema: getSchema()
  })

  // Reset form when tab changes
  useEffect(() => {
    reset(getDefaultValues())
  }, [selectedTab, reset])

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
      ...(data.managedDb && { managedDb: true }) // Assuming backend expects boolean
    }
    return mutate(projectData)
  }

  const success = !error && isSubmitted

  return (
    <>
      <PageHeader contained>
        <div className="flex items-center gap-3">
          <Button as="a" href={getRoutePath('projects')} variant="ghost" size="small">
            ← Back
          </Button>
          <h1 className="text-2xl font-bold">Create New Project</h1>
        </div>
      </PageHeader>

      <PageContent contained>
        <div className="mx-auto w-full max-w-2xl">
          <form
            {...props}
            className={cn('flex flex-col gap-6', className)}
            onSubmit={handleSubmit(handleFormSubmit)}
            autoComplete="off"
          >
            {/* Tab Navigation */}
            <div className="mb-6">
              <div className="border-border flex border-b">
                <button
                  type="button"
                  onClick={() => setSelectedTab('shared')}
                  className={cn(
                    'flex items-center gap-2 border-b-2 px-4 py-2 text-sm font-medium transition-colors',
                    selectedTab === 'shared' ? 'border-accent text-accent' : (
                      'text-content-secondary hover:text-content-primary border-transparent'
                    )
                  )}
                >
                  <Database className="h-4 w-4" />
                  Shared Instance
                </button>
                <button
                  type="button"
                  onClick={() => setSelectedTab('custom')}
                  className={cn(
                    'flex items-center gap-2 border-b-2 px-4 py-2 text-sm font-medium transition-colors',
                    selectedTab === 'custom' ? 'border-accent text-accent' : (
                      'text-content-secondary hover:text-content-primary border-transparent'
                    )
                  )}
                >
                  <Globe className="h-4 w-4" />
                  Connect Neo4j
                  {!hasValidSubscription && <SparklesIcon className="h-3 w-3" />}
                </button>
                <button
                  type="button"
                  onClick={() => setSelectedTab('managed')}
                  className={cn(
                    'flex items-center gap-2 border-b-2 px-4 py-2 text-sm font-medium transition-colors',
                    selectedTab === 'managed' ? 'border-accent text-accent' : (
                      'text-content-secondary hover:text-content-primary border-transparent'
                    )
                  )}
                >
                  <Cloud className="h-4 w-4" />
                  Managed Instance
                  {!hasValidSubscription && <SparklesIcon className="h-3 w-3" />}
                </button>
              </div>
            </div>

            {/* Basic Fields */}
            <div className="grid gap-4">
              <TextField
                label="Name"
                {...register('name')}
                autoFocus
                error={errors?.name?.message}
                data-tour="project-name-input"
                autoComplete="off"
              />
              <TextField
                label="Description (optional)"
                {...register('description')}
                error={errors?.description?.message}
                autoComplete="off"
              />
            </div>

            {/* Tab Content - Fixed height container to prevent jumping */}
            <div className="min-h-[300px]">
              {/* {selectedTab === 'shared' && (
                <div className="bg-surface-secondary rounded-lg p-4">
                  <h3 className="mb-2 font-semibold">Free Shared Instance</h3>
                  <div className="text-content-secondary space-y-2 text-sm">
                    <p>
                      Perfect for getting started with RushDB. Your project will use our shared Neo4j instance
                      with:
                    </p>
                    <ul className="ml-2 list-inside list-disc space-y-1">
                      <li>Up to 10,000 records</li>
                      <li>Guaranteed data isolation and privacy</li>
                      <li>Basic query capabilities</li>
                      <li>Community support</li>
                    </ul>
                  </div>
                </div>
              )} */}

              {selectedTab === 'custom' && (
                <>
                  <div className="mt-4">
                    <div className="bg-surface-secondary border-accent/20 mb-4 rounded-lg border p-4">
                      <h3 className="mb-2 font-semibold">Connect Your Own Neo4j Instance</h3>
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
                            className="w-full"
                          >
                            <SparklesIcon className="h-4 w-4" />
                            Upgrade for Premium Features
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
                        <p className="text-content-secondary text-xs">
                          Must contain at least 8 characters with uppercase, lowercase, number, and special
                          character
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

            <div className="flex justify-end">
              <Button
                loading={isSubmitting}
                type="submit"
                variant="accent"
                data-tour="create-project-btn"
                disabled={selectedTab === 'custom' && !hasValidSubscription}
                title={
                  selectedTab === 'custom' && !hasValidSubscription ?
                    'Upgrade to connect your own Neo4j instance'
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
