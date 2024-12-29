import { useStore } from '@nanostores/react'
import { FolderPlus, SearchX, ZapIcon } from 'lucide-react'

import type { Project, ProjectStats } from '~/features/projects/types'

import magnifier from '~/assets/Magnifier.png'
import { Button } from '~/elements/Button'
import { SearchInput } from '~/elements/Input'
import { NothingFound } from '~/elements/NothingFound'
import { PageHeader, PageTitle } from '~/elements/PageHeader'
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
import { $currentProjectIsEmpty } from '~/features/projects/stores/current-project'
import { isProjectEmpty } from '~/features/projects/utils'
import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace.ts'

// linear-gradient(140deg, var(--token-58d418b2-b5b9-4a63-ab63-d5a3e03b4fcb, rgb(213, 235, 119)) /* {"name":"Brand"} */ 0%, var(--token-25fcd9c3-5c51-4aaa-99d4-b1c3368900ae, rgb(132, 184, 76)) /* {"name":"Brand Gradient"} */ 100%)

const statsMap: Record<keyof ProjectStats, string> = {
  properties: 'Properties',
  records: 'Records'
}

function ProjectCard({ description, id, stats, name }: Project) {
  const isEmpty = isProjectEmpty({
    loading: false,
    totalRecords: stats?.records ?? 0
  })

  const href = isEmpty
    ? getRoutePath('projectHelp', { id })
    : getRoutePath('project', { id })

  return (
    <a
      className="interaction rounded-lg border border-transparent bg-secondary ring-interaction-ring focus-visible:border-interaction-focus focus-visible:ring [&:hover:not(:focus-visible)]:bg-secondary-hover"
      href={href}
    >
      <article className={cn('flex flex-col gap-3 p-5 transition')}>
        <header className="flex flex-col items-start justify-between">
          <h4 className="truncate text-lg font-bold text-content">{name}</h4>
          <p className={cn('text-sm text-content2')}>
            {description || 'No description provided'}
          </p>
        </header>
        <hr />
        <div className="flex justify-start gap-12">
          {stats &&
            Object.entries(stats).map(([label, value]) => (
              <Metric
                key={`${id}-stat-${label}-${value}`}
                label={
                  label in statsMap
                    ? statsMap[label as keyof typeof statsMap]
                    : label
                }
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

  return (
    <SearchInput
      placeholder="Search..."
      {...props}
      onChange={filterProjects}
      value={query}
    />
  )
}

function Header() {
  const { data: projects } = useStore($workspaceProjects)
  const showUpgradeButton = useStore($showUpgrade)

  return (
    <PageHeader className="justify-between gap-5">
      <PageTitle>
        Projects{' '}
        {projects?.length ? (
          <span className="ml-1 text-content2">{projects?.length}</span>
        ) : null}
      </PageTitle>
      {showUpgradeButton ? (
        <Button as="a" href={getRoutePath('workspaceBilling')} variant="accent">
          <ZapIcon />
          Upgrade Plan
        </Button>
      ) : (
        <Button as="a" href={getRoutePath('newProject')} variant="primary">
          <FolderPlus />
          New Project
        </Button>
      )}
    </PageHeader>
  )
}

function EmptyProjects() {
  return (
    <div className="mx-auto grid h-full w-full max-w-lg flex-1 place-content-center place-items-center gap-3 p-5">
      <SearchX size={64} />
      <h4 className="text-2xl font-bold">
        You haven&apos;t created any projects yet!
      </h4>
      <p className="text-content/90">
        Project is a place to organize related records
      </p>
      <Button
        as="a"
        className="mt-5 w-full"
        href={getRoutePath('newProject')}
        variant="accent"
      >
        New Project
      </Button>
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

  if (loading) {
    return <div className="grid flex-1 place-items-center">Loading...</div>
  }

  if (!loading && (!projects || projects?.length < 1)) {
    return <EmptyProjects />
  }

  return (
    <>
      <Header />
      <div className="flex flex-1 flex-col gap-5 px-5 pb-10">
        <ProjectsCards />
      </div>
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
