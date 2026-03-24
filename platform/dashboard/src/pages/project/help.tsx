import { PageContent, PageHeader } from '~/elements/PageHeader'
import {
  ClientLibrariesStep,
  ExploreDocsStep,
  UseSdkStep,
  WelcomeStep
} from '~/features/onboarding/components/steps'
import { useCurrentProjectQuery } from '~/features/projects/hooks/useProjectQueries'

export function ProjectHelpPage() {
  const { data, isPending: loading } = useCurrentProjectQuery()

  return (
    <>
      <PageHeader contained>
        <WelcomeStep loading={loading} />
      </PageHeader>

      <PageContent contained>
        <UseSdkStep projectId={data?.id} />

        <ExploreDocsStep />

        <ClientLibrariesStep />
      </PageContent>
    </>
  )
}
