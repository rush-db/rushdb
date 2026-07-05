import { useEffect } from 'react'

import { ExternalLink } from 'lucide-react'

import { PageHeader, PageTitle } from '~/elements/PageHeader'
import { RawApiView } from '~/features/projects/components/RawApiView'
import { $tourStep, setTourStep } from '~/features/tour/stores/tour'

const QUERY_LAB_DOCS_URL = 'https://docs.rushdb.com/concepts/search/introduction'

export function ProjectQueryLab() {
  useEffect(() => {
    // During onboarding, opening the Query Lab advances the tour from the
    // "open the lab" step into the in-lab walkthrough.
    if ($tourStep.get() === 'recordRawApiMode') {
      setTourStep('rawApiSelectQuery', true)
    }
  }, [])

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-auto">
      <PageHeader className="items-start">
        <div className="flex max-w-3xl flex-col gap-2">
          <PageTitle>Query Lab</PageTitle>
          <p className="text-sm leading-6 text-content2">
            Build and run SearchQuery payloads against this project, inspect the generated Cypher, and explore
            the raw records, properties, and labels the API returns.
          </p>
          <a
            className="inline-flex w-fit items-center gap-2 text-sm text-content2 transition hover:text-content"
            href={QUERY_LAB_DOCS_URL}
            rel="noreferrer"
            target="_blank"
          >
            Read the docs <ExternalLink className="h-3.5 w-3.5" />
          </a>
        </div>
      </PageHeader>
      <RawApiView />
    </div>
  )
}
