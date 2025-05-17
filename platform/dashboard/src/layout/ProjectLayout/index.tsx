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

function ProjectRoutes({ id }: { id: Project['id'] }) {
  const page = useStore($router)

  switch (page?.route) {
    case 'projectTokens':
      return <ProjectTokens projectId={id} />
    case 'projectSettings':
      return <ProjectSettings projectId={id} />
    case 'projectImportData':
      return <ImportRecords />
    case 'projectHelp':
      return <ProjectHelpPage />
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

  if (!data && loading === false) {
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
      <ProjectTabs projectId={projectId} />
      <ProjectRoutes id={projectId} />
      <PropertyValueTooltip />
    </>
  )
}
