import { ArrowLeft, Cable } from 'lucide-react'
import { useStore } from '@nanostores/react'

import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { ConnectorSetupWizard } from '~/features/connectors/components/ConnectorSetupWizard'
import { useConnectorCatalogQuery } from '~/features/connectors/hooks'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { $router, getRoutePath, redirectRoute } from '~/lib/router'

const DB_TYPES = ['postgres', 'mysql', 'mongodb']

export function ProjectNewConnection({ projectId }: { projectId: Project['id'] }) {
  const page = useStore($router)
  const { data: platformSettings, isPending: settingsPending } = usePlatformSettings()
  const { data: catalog } = useConnectorCatalogQuery()
  const rawType =
    page?.route === 'projectNewConnection' ? (page.params.sourceType as string | undefined) : undefined

  // Accept database types always, plus any connector the registered catalog
  // declares (spec connectors like hubspot/salesforce are never hardcoded).
  const catalogTypes = new Set([...DB_TYPES, ...(catalog?.connectors.map((c) => c.id) ?? [])])
  const sourceType: string | null = rawType && catalogTypes.has(rawType) ? rawType : null

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
          <PageTitle>
            Connect {DB_TYPES.includes(sourceType) ? sourceType : sourceType.toUpperCase()}
          </PageTitle>
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
