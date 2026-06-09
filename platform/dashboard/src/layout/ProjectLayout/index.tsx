import { useEffect } from 'react'
import { useStore } from '@nanostores/react'

import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { NothingFound } from '~/elements/NothingFound'
import { Spinner } from '~/elements/Spinner'

import { ProjectTabs } from '~/features/projects/components/ProjectTabs'
import { useCurrentProjectQuery } from '~/features/projects/hooks/useProjectQueries'
import { PropertyValueTooltip } from '~/features/properties/components/PropertyValueTooltip'
import { $router, getRoutePath, isProjectPage, redirectRoute } from '~/lib/router'

import { ProjectSettings } from '~/pages/project/settings'
import { ProjectTokens } from '~/pages/project/tokens'
import { ProjectIndexes } from '~/pages/project/indexes'
import { ProjectRelationships } from '~/pages/project/relationships'
import { ProjectRecordsPage } from '~/pages/project/records'
import { ProjectHelpPage } from '~/pages/project/help'
import { ImportRecords } from '~/features/records/components/ImportRecords.tsx'

function ProjectRoutes({ project }: { project: Project }) {
  const page = useStore($router)

  switch (page?.route) {
    case 'projectTokens':
      return <ProjectTokens projectId={project.id} />
    case 'projectIndexes':
      return <ProjectIndexes projectId={project.id} />
    case 'projectRelationships':
      return <ProjectRelationships projectId={project.id} />
    case 'projectSettings':
      return <ProjectSettings projectId={project.id} />
    case 'projectImportData':
      return <ImportRecords />
    case 'projectHelp':
      return <ProjectHelpPage />
    default:
      return <ProjectRecordsPage />
  }
}

const PROJECT_TAB_TITLES: Record<string, string> = {
  project: 'Records',
  projectSettings: 'Settings',
  projectTokens: 'API Tokens',
  projectIndexes: 'Indexes',
  projectRelationships: 'Relationships',
  projectImportData: 'Import',
  projectUsers: 'Users',
  projectHelp: 'Getting Started',
  projectBilling: 'Billing'
}

export function ProjectLayout() {
  const page = useStore($router)

  const projectId = isProjectPage(page) ? page?.params.id : undefined

  const { data, isPending } = useCurrentProjectQuery()

  useEffect(() => {
    if (!data) return
    const tab = (page?.route && PROJECT_TAB_TITLES[page.route]) ?? 'Records'
    document.title = `${data.name} / ${tab} – RushDB`
  }, [data, page?.route])

  if (!projectId) {
    redirectRoute('projects')

    return null
  }

  if (isPending) {
    return (
      <div className="grid flex-1 place-items-center">
        <Spinner />
      </div>
    )
  }

  if (!data) {
    return (
      <NothingFound
        action={
          <Button as="a" href={getRoutePath('projects')} variant="accent">
            Go to projects
          </Button>
        }
        title="Project not found"
      />
    )
  }

  return (
    <>
      <ProjectTabs project={data!} />
      <ProjectRoutes project={data!} />
      <PropertyValueTooltip />
    </>
  )
}
