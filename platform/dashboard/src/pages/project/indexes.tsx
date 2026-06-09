import type { Project } from '~/features/projects/types'

import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { useProjectIndexesQuery } from '~/features/projects/hooks/useProjectQueries'
import { AddIndexCard } from '~/features/indexes/components/AddIndex'
import { IndexesList } from '~/features/indexes/components/IndexesList'
import { SuggestedIndexesCard } from '~/features/indexes/components/SuggestedIndexes'

export function ProjectIndexes({ projectId }: { projectId: Project['id'] }) {
  const { data: indexes, isPending: loading } = useProjectIndexesQuery()

  return (
    <>
      <PageHeader contained>
        <PageTitle>Embedding Indexes</PageTitle>
      </PageHeader>
      <PageContent contained>
        <SuggestedIndexesCard existingIndexes={indexes} indexesLoading={loading} projectId={projectId} />
        <AddIndexCard existingIndexes={indexes} projectId={projectId} />
        <IndexesList data={indexes} loading={loading} />
      </PageContent>
    </>
  )
}
