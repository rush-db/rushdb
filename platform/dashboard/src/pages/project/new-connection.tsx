import { ArrowLeft, Cable } from 'lucide-react'
import { useStore } from '@nanostores/react'

import type { Project } from '~/features/projects/types'
import type { ConnectorType } from '~/features/connectors/types'

import { Button } from '~/elements/Button'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { ConnectorSetupWizard } from '~/features/connectors/components/ConnectorSetupWizard'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { $router, getRoutePath, redirectRoute } from '~/lib/router'

function parseSourceType(value?: string): ConnectorType | null {
  if (value === 'postgres' || value === 'mongodb') return value
  return null
}

export function ProjectNewConnection({ projectId }: { projectId: Project['id'] }) {
  const page = useStore($router)
  const { data: platformSettings, isPending: settingsPending } = usePlatformSettings()
  const sourceType = parseSourceType(
    page?.route === 'projectNewConnection' ? page.params.sourceType : undefined
  )

  if (!settingsPending && !platformSettings?.synxEnabled) {
    redirectRoute('projectImportData', { id: projectId })
    return null
  }

  if (!sourceType) {
    redirectRoute('projectImportData', { id: projectId })
    return null
  }

  return (
    <>
      <PageHeader contained>
        <div className="flex items-center gap-3">
          <Cable />
          <PageTitle>Connect {sourceType === 'postgres' ? 'PostgreSQL' : 'MongoDB'}</PageTitle>
        </div>
        <Button
          as="a"
          href={getRoutePath('projectImportData', { id: projectId })}
          size="small"
          variant="outline"
        >
          <ArrowLeft size={16} />
          Import data
        </Button>
      </PageHeader>
      <PageContent contained>
        <ConnectorSetupWizard
          sourceType={sourceType}
          onClose={() => redirectRoute('projectImportData', { id: projectId })}
        />
      </PageContent>
    </>
  )
}
