import { ExternalLink } from 'lucide-react'

import { Button } from '~/elements/Button'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Skeleton } from '~/elements/Skeleton'
import { ConnectGuide, ProjectConnectionPanel } from '~/features/connect-guide'
import { connectDocsUrls } from '~/features/connect-guide/constants'
import { ExploreDocsStep } from '~/features/onboarding/components/steps'
import { useCurrentProjectQuery, useFilteredRecordsQuery } from '~/features/projects/hooks/useProjectQueries'
import { isProjectEmpty } from '~/features/projects/utils'

export function ProjectHelpPage() {
  const { data: project, isPending: loading } = useCurrentProjectQuery()
  const { data: recordsResult, isPending: recordsLoading } = useFilteredRecordsQuery()
  const isEmpty = isProjectEmpty({ totalRecords: recordsResult?.total, loading: recordsLoading })

  return (
    <>
      <PageHeader contained className="items-start">
        <div className="flex min-w-0 flex-col gap-2">
          <Skeleton enabled={loading}>
            <PageTitle>Connect this project</PageTitle>
          </Skeleton>
          <p className="text-content2">
            {isEmpty ?
              'Your project is ready. Choose how you want to use RushDB.'
            : 'Choose how you want to use RushDB — from apps, agents, or code.'}
          </p>
        </div>

        <Button
          as="a"
          href={connectDocsUrls.overview}
          rel="noopener noreferrer"
          size="small"
          target="_blank"
          variant="outline"
        >
          Documentation
          <ExternalLink />
        </Button>
      </PageHeader>

      <PageContent contained>
        <ProjectConnectionPanel loading={loading} project={project} />
        <ConnectGuide projectId={project?.id} />
        <ExploreDocsStep />
      </PageContent>
    </>
  )
}
