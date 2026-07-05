import { ExternalLink, Plus } from 'lucide-react'

import { Button } from '~/elements/Button'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import { useWorkspaceProjectsQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { useWorkspaceTokensQuery } from '~/features/tokens/hooks/useTokenMutations'
import { AddWorkspaceTokenDialog } from '~/features/tokens/components/AddWorkspaceToken'
import { WorkspaceTokensList } from '~/features/tokens/components/WorkspaceTokensList'

const API_KEYS_DOCS_URL = 'https://docs.rushdb.com/deploy/configuration/get-api-key'

export function WorkspaceApiKeysPage() {
  const { data: projects } = useWorkspaceProjectsQuery()
  const { data: tokens, isPending: loading } = useWorkspaceTokensQuery()
  const projectList = projects ?? []

  return (
    <WorkspacesLayout>
      <PageHeader className="items-start" contained>
        <div className="flex max-w-3xl flex-col gap-2">
          <PageTitle>API Keys</PageTitle>
          <p className="text-sm leading-6 text-content2">
            API keys authenticate requests to your projects from the RushDB SDKs and REST API. Manage the keys
            for every project in this workspace from one place.
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
        <AddWorkspaceTokenDialog
          projects={projectList}
          trigger={
            <Button disabled={!projectList.length} variant="primary">
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
              All keys across the projects in this workspace, and the project each one can access.
            </p>
          </div>
          <WorkspaceTokensList data={tokens} loading={loading} />
        </section>
      </PageContent>
    </WorkspacesLayout>
  )
}
