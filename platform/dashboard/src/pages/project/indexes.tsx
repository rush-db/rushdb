import type { Project } from '~/features/projects/types'

import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { useProjectIndexesQuery } from '~/features/projects/hooks/useProjectQueries'
import { AddIndexCard } from '~/features/indexes/components/AddIndex'
import { IndexesList } from '~/features/indexes/components/IndexesList'

export function ProjectIndexes({ projectId }: { projectId: Project['id'] }) {
  const { data: indexes, isPending: loading } = useProjectIndexesQuery()

  return (
    <>
      <PageHeader contained>
        <PageTitle>Embedding Indexes</PageTitle>
      </PageHeader>
      <PageContent contained>
        <AddIndexCard projectId={projectId} />
        <IndexesList data={indexes} loading={loading} />
      </PageContent>
    </>
  )
}
