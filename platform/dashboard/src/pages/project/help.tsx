import { useStore } from '@nanostores/react'
import { PageContent, PageHeader } from '~/elements/PageHeader'
import {
  ClientLibrariesStep,
  ExploreDocsStep,
  UseSdkStep,
  WelcomeStep
} from '~/features/onboarding/components/steps'
import { $currentProject } from '~/features/projects/stores/current-project'

export function ProjectHelpPage() {
  const { data, loading } = useStore($currentProject)

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
