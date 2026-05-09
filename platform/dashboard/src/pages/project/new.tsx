import { Button } from '~/elements/Button'
import { PageContent, PageHeader } from '~/elements/PageHeader'
import { TextField } from '~/elements/Input'
import { FormField } from '~/elements/FormField'
import { useCreateProjectMutation } from '~/features/projects/hooks/useProjectMutations'
import { ArrowRight, SparklesIcon, Database, Cloud, Globe } from 'lucide-react'
import { object, string, useForm } from '~/lib/form'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'
import { useCurrentWorkspaceQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { useWorkspaceProjectsQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'

import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { setTourStep } from '~/features/tour/stores/tour.ts'
import { useEffect, useState, useMemo } from 'react'
import { useWatch } from 'react-hook-form'
import { Label } from '~/elements/Label'

// Type for form values
type ProjectFormValues = {
  name: string
  description?: string
  dataSource: 'shared' | 'custom'
  customDb?: {
    url: string
    username: string
    password: string
  }
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

function CreateProjectForm({ className, ...props }: TPolymorphicComponentProps<'form'>) {
  const { mutateAsync: mutate, error } = useCreateProjectMutation()
  const { data: workspace } = useCurrentWorkspaceQuery()
  const { data: platformSettings, isPending: loading } = usePlatformSettings()
  const { data: projects, isFetching: isProjectsFetching } = useWorkspaceProjectsQuery()
  const maxProjects = workspace?.projectLimit ?? null
  const showUpgradeButton = useMemo(() => {
    if (isProjectsFetching) {
      return false
    }

    return !platformSettings?.selfHosted && maxProjects !== null && typeof projects !== 'undefined' ?
        (projects?.length ?? 0) >= maxProjects
      : false
  }, [platformSettings, maxProjects, projects, isProjectsFetching])

  const [selectedTab, setSelectedTab] = useState<'shared' | 'custom'>('shared')

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

  const handleFormSubmit = (data: ProjectFormValues) => {
    // Transform data to match the expected Project type
    const projectData = {
      name: data.name,
      description: data.description,
      ...(data.customDb && { customDb: data.customDb })
    }
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
                    Hosted Database <span className="text-content2 font-normal">[Fully Managed]</span>
                  </h3>
                  <p className="text-content2 text-sm">
                    Start building instantly with our managed Neo4j infrastructure. Perfect for getting
                    started, prototypes, and production apps. Zero setup required.
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
                  </div>
                  <h3 className="mb-1 text-lg font-semibold leading-snug">
                    Your Own Database{' '}
                    <span className="text-content2 font-normal">[Bring Your Own Neo4j]</span>
                  </h3>
                  <p className="text-content2 text-sm">
                    Connect your existing Neo4j instance (Aura or self-hosted). Full control over your data
                    and infrastructure. Unlimited custom queries.
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
                disabled={showUpgradeButton || isSubmitting}
              />
              <TextField
                label="Description (optional)"
                {...register('description')}
                error={errors?.description?.message}
                autoComplete="off"
                disabled={showUpgradeButton || isSubmitting}
              />
            </div>

            {selectedTab === 'custom' && (
              <div className="">
                <div className="mt-4">
                  <div>
                    <FormField label="Database URL" error={errors?.customDb?.url?.message}>
                      <TextField
                        {...register('customDb.url')}
                        placeholder="bolt://your-database-url:7687"
                        autoComplete="off"
                        inputMode="url"
                        disabled={showUpgradeButton || isSubmitting}
                      />
                    </FormField>
                    <FormField label="Username" error={errors?.customDb?.username?.message}>
                      <TextField
                        {...register('customDb.username')}
                        placeholder="neo4j"
                        autoComplete="off"
                        disabled={showUpgradeButton || isSubmitting}
                      />
                    </FormField>
                    <FormField label="Password" error={errors?.customDb?.password?.message}>
                      <TextField
                        type="password"
                        {...register('customDb.password')}
                        placeholder="your-password"
                        autoComplete="new-password"
                        spellCheck={false}
                        disabled={showUpgradeButton || isSubmitting}
                      />
                    </FormField>
                  </div>
                </div>
              </div>
            )}

            {showUpgradeButton && (
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

            <div className="flex items-center justify-end gap-4">
              <Button as="a" href={getRoutePath('projects')} variant="secondary">
                Cancel
              </Button>
              <Button
                loading={isSubmitting}
                type="submit"
                variant="accent"
                data-tour="create-project-btn"
                disabled={showUpgradeButton}
                title={showUpgradeButton ? 'Upgrade plan to create more projects' : undefined}
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
