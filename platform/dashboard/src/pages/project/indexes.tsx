import type { Project } from '~/features/projects/types'

import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { useProjectIndexesQuery } from '~/features/projects/hooks/useProjectQueries'
import { AddIndexDialog } from '~/features/indexes/components/AddIndex'
import { IndexesList } from '~/features/indexes/components/IndexesList'
import { SuggestedIndexesCard } from '~/features/indexes/components/SuggestedIndexes'
import { NothingFound } from '~/elements/NothingFound'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { Button } from '~/elements/Button'
import { ExternalLink, Plus } from 'lucide-react'

const EMBEDDING_INDEX_DOCS_URL = 'https://docs.rushdb.com/learn/semantic-search/manage-indexes'

export function ProjectIndexes({ projectId }: { projectId: Project['id'] }) {
  const { data: indexes, isPending: loading } = useProjectIndexesQuery()
  const { data: platformSettings, isPending: settingsPending } = usePlatformSettings()
  const embeddingEnabled = platformSettings?.embeddingEnabled === true

  return (
    <>
      <PageHeader className="items-start" contained>
        <div className="flex max-w-3xl flex-col gap-2">
          <PageTitle>Semantic Indexes</PageTitle>
          <p className="text-content2 text-sm leading-6">
            Semantic indexes make selected text fields searchable by meaning. Each index is scoped to one
            label and property, backfills matching records, and powers semantic search from the Records page
            and API.
          </p>
          <a
            className="text-content2 hover:text-content inline-flex w-fit items-center gap-2 text-sm transition"
            href={EMBEDDING_INDEX_DOCS_URL}
            rel="noreferrer"
            target="_blank"
          >
            Read the docs <ExternalLink className="h-3.5 w-3.5" />
          </a>
        </div>
        {settingsPending || embeddingEnabled ?
          <AddIndexDialog
            existingIndexes={indexes}
            projectId={projectId}
            trigger={
              <Button variant="primary">
                <Plus />
                New Index
              </Button>
            }
          />
        : null}
      </PageHeader>
      <PageContent className="gap-8" contained>
        {settingsPending || embeddingEnabled ?
          <>
            <section className="flex flex-col gap-3">
              <div>
                <h2 className="text-content text-lg font-semibold">Created indexes</h2>
                <p className="text-content2 text-sm">
                  Monitor index status and backfill progress for searchable text fields in this project.
                </p>
              </div>
              <IndexesList data={indexes} loading={loading} />
            </section>
            <section className="flex flex-col gap-3">
              <SuggestedIndexesCard
                existingIndexes={indexes}
                indexesLoading={loading}
                projectId={projectId}
              />
            </section>
          </>
        : <NothingFound title="Semantic search is disabled" />}
      </PageContent>
    </>
  )
}
