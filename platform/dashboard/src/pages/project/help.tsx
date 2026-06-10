import { PageContent, PageHeader } from '~/elements/PageHeader'
import { ConnectGuide } from '~/features/connect-guide'
import { ClientLibrariesStep, ExploreDocsStep, WelcomeStep } from '~/features/onboarding/components/steps'
import { useCurrentProjectQuery } from '~/features/projects/hooks/useProjectQueries'

export function ProjectHelpPage() {
  const { data, isPending: loading } = useCurrentProjectQuery()

  return (
    <>
      <PageHeader contained>
        <WelcomeStep loading={loading} />
      </PageHeader>

      <PageContent contained>
        <ConnectGuide projectId={data?.id} />
        <ExploreDocsStep />
        <ClientLibrariesStep />
      </PageContent>
    </>
  )
}
