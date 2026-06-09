import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { ConnectGuide } from '~/features/connect-guide'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'

export function WorkspaceGettingStartedPage() {
  return (
    <WorkspacesLayout>
      <PageHeader contained>
        <PageTitle>Getting Started</PageTitle>
      </PageHeader>

      <PageContent contained>
        <ConnectGuide />
      </PageContent>
    </WorkspacesLayout>
  )
}
