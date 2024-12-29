import { useStore } from '@nanostores/react'

import type { Project } from '~/features/projects/types'

import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import {
  $currentProject,
  $currentProjectTokens
} from '~/features/projects/stores/current-project'
import { AddTokenCard } from '~/features/tokens/components/AddToken'
import { TokensList } from '~/features/tokens/components/TokensList'

export function ProjectTokens({ projectId }: { projectId: Project['id'] }) {
  const { data: project } = useStore($currentProject)
  const { data: tokens, loading } = useStore($currentProjectTokens)

  return (
    <>
      <PageHeader contained>
        <PageTitle>API Keys</PageTitle>
      </PageHeader>
      <PageContent contained>
        <AddTokenCard project={project} projectId={projectId} />
        <TokensList data={tokens} loading={loading} />
      </PageContent>
    </>
  )
}
