import { useStore } from '@nanostores/react'
import { FolderPlus, SearchX, ZapIcon } from 'lucide-react'

import type { Project, ProjectStats } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { SearchInput } from '~/elements/Input'
import { NothingFound } from '~/elements/NothingFound'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Metric } from '~/features/projects/components'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import {
  $filteredProjects,
  $projectsQuery,
  $showUpgrade,
  $workspaceProjects,
  filterProjects
} from '~/features/workspaces/stores/projects'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'
import { isProjectEmpty } from '~/features/projects/utils'
import { useEffect } from 'react'
import { api } from '~/lib/api.ts'
import { $user } from '~/features/auth/stores/user.ts'
import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace.ts'

const statsMap: Record<keyof ProjectStats, string> = {
  properties: 'Properties',
  records: 'Records'
}

function ProjectCard({ description, id, stats, name }: Project) {
  const isEmpty = isProjectEmpty({
    loading: false,
    totalRecords: stats?.records ?? 0
  })

  const href = isEmpty ? getRoutePath('projectHelp', { id }) : getRoutePath('project', { id })

  return (
    <a
      className="interaction bg-secondary ring-interaction-ring focus-visible:border-interaction-focus [&:hover:not(:focus-visible)]:bg-secondary-hover rounded-lg border border-transparent focus-visible:ring"
      href={href}
    >
      <article className={cn('flex flex-col gap-3 p-5 transition')}>
        <header className="flex flex-col items-start justify-between">
          <h4 className="text-content truncate text-lg font-bold">{name}</h4>
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

function ProjectsSearchInput(props: TInheritableElementProps<'input'>) {
  const query = useStore($projectsQuery)

  return <SearchInput placeholder="Search..." {...props} onChange={filterProjects} value={query} />
}

function Header() {
  const { data: projects } = useStore($workspaceProjects)
  const showUpgradeButton = useStore($showUpgrade)
  const currentUser = useStore($user)
  const isOwner = currentUser.currentScope?.role === 'owner'

  return (
    <PageHeader className="justify-between gap-5" contained>
      <PageTitle>
        Projects{' '}
        {projects?.length ?
          <span className="text-content2 ml-1">{projects?.length}</span>
        : null}
      </PageTitle>
      {showUpgradeButton && isOwner ?
        <Button as="a" href={getRoutePath('workspaceBilling')} variant="accent">
          <ZapIcon />
          Upgrade Plan
        </Button>
      : isOwner ?
        <Button as="a" href={getRoutePath('newProject')} variant="primary">
          <FolderPlus />
          New Project
        </Button>
      : null}
    </PageHeader>
  )
}

function EmptyProjects() {
  const currentUser = useStore($user)
  const isOwner = currentUser.currentScope?.role === 'owner'

  return (
    <div className="mx-auto grid h-full w-full max-w-lg flex-1 place-content-center place-items-center gap-3 p-5">
      <SearchX size={64} />
      <h4 className="text-2xl font-bold">You haven&apos;t created any projects yet!</h4>
      <p className="text-content/90">Project is a place to organize related records</p>
      {isOwner ?
        <Button
          as="a"
          className="mt-5 w-full"
          data-tour="new-project-btn"
          href={getRoutePath('newProject')}
          variant="accent"
        >
          New Project
        </Button>
      : null}
    </div>
  )
}

function ProjectsCards() {
  const filteredProjects = useStore($filteredProjects)

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
  const { data: projects, loading } = useStore($workspaceProjects)
  const { data: workspace } = useStore($currentWorkspace)

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
        <div className="flex flex-1 flex-col gap-5 pb-10">
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
