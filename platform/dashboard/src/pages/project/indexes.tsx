import { useStore } from '@nanostores/react'

import type { Project } from '~/features/projects/types'

import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { $currentProjectIndexes } from '~/features/projects/stores/current-project'
import { AddIndexCard } from '~/features/indexes/components/AddIndex'
import { IndexesList } from '~/features/indexes/components/IndexesList'

export function ProjectIndexes({ projectId }: { projectId: Project['id'] }) {
  const { data: indexes, loading } = useStore($currentProjectIndexes)

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
