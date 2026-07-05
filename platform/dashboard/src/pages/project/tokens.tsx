import { ExternalLink, Plus } from 'lucide-react'

import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { useCurrentProjectQuery, useProjectTokensQuery } from '~/features/projects/hooks/useProjectQueries'
import { AddTokenDialog } from '~/features/tokens/components/AddToken'
import { TokensList } from '~/features/tokens/components/TokensList'

const API_KEYS_DOCS_URL = 'https://docs.rushdb.com/deploy/configuration/get-api-key'

export function ProjectTokens({ projectId }: { projectId: Project['id'] }) {
  const { data: project } = useCurrentProjectQuery()
  const { data: tokens, isPending: loading } = useProjectTokensQuery()

  return (
    <>
      <PageHeader className="items-start" contained>
        <div className="flex max-w-3xl flex-col gap-2">
          <PageTitle>API Keys</PageTitle>
          <p className="text-sm leading-6 text-content2">
            API keys authenticate requests to this project from the RushDB SDKs and REST API. Create a key per
            app or environment, set an optional expiration, and revoke any key at any time.
          </p>
          <a
            className="inline-flex w-fit items-center gap-2 text-sm text-content2 transition hover:text-content"
            href={API_KEYS_DOCS_URL}
            rel="noreferrer"
            target="_blank"
          >
            Read the docs <ExternalLink className="h-3.5 w-3.5" />
          </a>
        </div>
        <AddTokenDialog
          project={project}
          projectId={projectId}
          trigger={
            <Button variant="primary">
              <Plus />
              New API Key
            </Button>
          }
        />
      </PageHeader>
      <PageContent className="gap-8" contained>
        <section className="flex flex-col gap-3">
          <div>
            <h2 className="text-lg font-semibold text-content">Your API keys</h2>
            <p className="text-sm text-content2">
              Manage the keys that can read from and write to this project.
            </p>
          </div>
          <TokensList data={tokens} loading={loading} />
        </section>
      </PageContent>
    </>
  )
}
