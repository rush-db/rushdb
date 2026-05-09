import type { Project } from '~/features/projects/types'

import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { useCurrentProjectQuery, useProjectTokensQuery } from '~/features/projects/hooks/useProjectQueries'
import { AddTokenCard } from '~/features/tokens/components/AddToken'
import { TokensList } from '~/features/tokens/components/TokensList'

export function ProjectTokens({ projectId }: { projectId: Project['id'] }) {
  const { data: project } = useCurrentProjectQuery()
  const { data: tokens, isPending: loading } = useProjectTokensQuery()

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
