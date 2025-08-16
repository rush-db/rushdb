import { useStore } from '@nanostores/react'

import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { NothingFound } from '~/elements/NothingFound'

import { ProjectTabs } from '~/features/projects/components/ProjectTabs'
import { $currentProject } from '~/features/projects/stores/current-project'
import { PropertyValueTooltip } from '~/features/properties/components/PropertyValueTooltip'
import { $router, getRoutePath, isProjectPage, redirectRoute } from '~/lib/router'

import { ProjectSettings } from '~/pages/project/settings'
import { ProjectTokens } from '~/pages/project/tokens'
import { ProjectRecordsPage } from '~/pages/project/records'
import { ProjectHelpPage } from '~/pages/project/help'
import { ImportRecords } from '~/features/records/components/ImportRecords.tsx'
import { ProjectBillingPage } from '~/pages/project/billing.tsx'

function ProjectRoutes({ project }: { project: Project }) {
  const page = useStore($router)

  switch (page?.route) {
    case 'projectTokens':
      return <ProjectTokens projectId={project.id} />
    case 'projectSettings':
      return <ProjectSettings projectId={project.id} />
    case 'projectImportData':
      return <ImportRecords />
    case 'projectHelp':
      return <ProjectHelpPage />
    case 'projectBilling':
      return project.managedDb ? <ProjectBillingPage /> : null
    default:
      return <ProjectRecordsPage />
  }
}

export function ProjectLayout() {
  const page = useStore($router)

  const projectId = isProjectPage(page) ? page?.params.id : undefined

  const { data, loading } = useStore($currentProject)

  if (!projectId) {
    redirectRoute('projects')

    return null
  }

  if (!data || loading) {
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
