import { ExternalLink, Plus } from 'lucide-react'

import { Button } from '~/elements/Button'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'

import { useImportRunsQuery } from '../hooks/useImportQueries'
import { ImportsList } from '../components/ImportsList'
import { getRoutePath } from '~/lib/router'
import { useStore } from '@nanostores/react'
import { $currentProjectId } from '~/features/projects/stores/id'

const IMPORTS_DOCS_URL = 'https://docs.rushdb.com'

export function ImportsPage() {
  const { data: runs, isPending: loading } = useImportRunsQuery()
  const projectId = useStore($currentProjectId)

  return (
    <>
      <PageHeader className="items-start" contained>
        <div className="flex max-w-3xl flex-col gap-2">
          <PageTitle>Imports</PageTitle>
          <p className="text-sm leading-6 text-content2">
            Upload multiple CSV, JSON, JSONL, NDJSON, or Parquet files as a single import run. Each file is
            parsed and committed asynchronously, with per-file progress, and you can define how record files
            and relationship files map into the graph.
          </p>
          <a
            className="inline-flex w-fit items-center gap-2 text-sm text-content2 transition hover:text-content"
            href={IMPORTS_DOCS_URL}
            rel="noreferrer"
            target="_blank"
          >
            Read the docs <ExternalLink className="h-3.5 w-3.5" />
          </a>
        </div>
        <Button as="a" href={getRoutePath('projectImportsNew', { id: projectId! })} variant="primary">
          <Plus />
          New import
        </Button>
      </PageHeader>
      <PageContent className="gap-8" contained>
        <section className="flex flex-col gap-3">
          <div>
            <h2 className="text-lg font-semibold text-content">Import runs</h2>
            <p className="text-sm text-content2">Monitor and manage multi-file imports for this project.</p>
          </div>
          <ImportsList data={runs} loading={loading} />
        </section>
      </PageContent>
    </>
  )
}
