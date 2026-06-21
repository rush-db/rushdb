import { useStore } from '@nanostores/react'
import { FolderPlus, Link, SearchX } from 'lucide-react'

import type { Project, ProjectStats } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { NothingFound } from '~/elements/NothingFound'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Metric } from '~/features/projects/components'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import { $projectsQuery } from '~/features/workspaces/stores/projects'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'
import { isProjectEmpty } from '~/features/projects/utils'
import { useEffect, useMemo } from 'react'
import { api } from '~/lib/api.ts'
import { $user } from '~/features/auth/stores/user.ts'
import { Label } from '~/elements/Label.tsx'
import {
  useCurrentWorkspaceQuery,
  useWorkspaceProjectsQuery
} from '~/features/workspaces/hooks/useWorkspaceQueries'

const statsMap: Record<keyof ProjectStats, string> = {
  properties: 'Properties',
  records: 'Records'
}

function ProjectCard({ description, id, stats, name, customDb, status }: Project & { customDb?: string }) {
  const isEmpty = isProjectEmpty({
    loading: false,
    totalRecords: stats?.records ?? 0
  })

  const projectIsInactive = status === 'pending' || status === 'provisioning'

  const href = useMemo(() => {
    if (isEmpty) {
      if (projectIsInactive) {
        return getRoutePath('projectSettings', { id })
      }
      return getRoutePath('projectHelp', { id })
    }
    return getRoutePath('project', { id })
  }, [projectIsInactive, isEmpty, id])

  return (
    <a
      className="interaction bg-secondary ring-interaction-ring focus-visible:border-interaction-focus [&:hover:not(:focus-visible)]:bg-secondary-hover rounded-lg border border-transparent focus-visible:ring"
      href={href}
    >
      <article className={cn('flex flex-col gap-3 p-5 transition')}>
        <header className="flex flex-col items-start justify-between">
          <div className="flex w-full items-center justify-between">
            <h4 className="text-content truncate text-lg font-bold">{name}</h4>
            {customDb && (
              <Label className="items-center !text-sm">
                <Link size={18} className="pr-2" />
                External Instance
              </Label>
            )}
          </div>
          <p className={cn('text-content2 text-sm')}>{description || 'No description provided'}</p>
        </header>
        <hr />
        <div className="flex justify-start gap-12">
          {stats &&
            Object.entries(stats).map(([label, value]) => (
              <Metric
                key={`${id}-stat-${label}-${value}`}
                label={label in statsMap ? statsMap[label as keyof typeof statsMap] : label}
                value={value}
              />
            ))}
        </div>
      </article>
    </a>
  )
}

function Header() {
  const { data: projects } = useWorkspaceProjectsQuery()

  return (
    <PageHeader className="items-start justify-between gap-5" contained>
      <div className="flex max-w-3xl flex-col gap-2">
        <PageTitle>
          Projects{' '}
          {projects?.length ?
            <span className="text-content2 ml-1">{projects?.length}</span>
          : null}
        </PageTitle>
        <p className="text-content2 text-sm leading-6">
          Projects are isolated data spaces for separate apps, customers, environments, or experiments.
          Records, relationships, indexes, API keys, and access rules stay scoped to the project they belong
          to, so each project can evolve without mixing data with another one.
        </p>
      </div>
      <Button data-tour="new-project-btn" as="a" href={getRoutePath('newProject')} variant="primary">
        <FolderPlus />
        New Project
      </Button>
    </PageHeader>
  )
}

function EmptyProjects() {
  const currentUser = useStore($user)
  const isOwner = currentUser.currentScope?.role === 'owner'

  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 p-8">
      <SearchX size={48} className="text-content2" />
      <h4 className="text-center text-xl font-bold">No projects yet</h4>
      <p className="text-content2 max-w-md text-center text-sm">
        Create a dedicated data space for your first application, customer, or workflow.
      </p>
      {isOwner && (
        <Button
          data-tour="new-project-btn"
          as="a"
          className="mt-3"
          href={getRoutePath('newProject')}
          variant="accent"
        >
          <FolderPlus />
          New Project
        </Button>
      )}
    </div>
  )
}

function ProjectsCards() {
  const { data: projects } = useWorkspaceProjectsQuery()
  const query = useStore($projectsQuery)
  const filteredProjects = useMemo(() => {
    const q = query?.toLowerCase() ?? ''
    return projects?.filter((p) => (p.name ?? '').toLowerCase().includes(q))
  }, [projects, query])

  if (!filteredProjects || filteredProjects?.length < 1) {
    return <NothingFound />
  }

  return (
    <>
      {filteredProjects.map((p) => (
        <ProjectCard key={p.id} {...p} />
      ))}
    </>
  )
}

function Projects() {
  const { data: projects, isPending: loading } = useWorkspaceProjectsQuery()
  const { data: workspace } = useCurrentWorkspaceQuery()

  useEffect(() => {
    if (typeof projects !== 'undefined') {
      api.user.current().then((res) => {
        $user.set({ ...$user.get(), ...res })
      })
    }
  }, [projects, workspace])

  if (loading) {
    return <div className="grid flex-1 place-items-center">Loading...</div>
  }

  if (!loading && (!projects || projects?.length < 1)) {
    return <EmptyProjects />
  }

  return (
    <>
      <Header />

      <PageContent contained>
        <div className="grid grid-cols-1 gap-5 pb-10 lg:grid-cols-2">
          <ProjectsCards />
        </div>
      </PageContent>
    </>
  )
}

export function WorkspaceProjectsPage() {
  return (
    <WorkspacesLayout>
      <Projects />
    </WorkspacesLayout>
  )
}
