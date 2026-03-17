import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import { KuUsageHistory } from '~/components/billing/KuUsageHistory.tsx'

export function WorkspaceApiUsagePage() {
  return (
    <WorkspacesLayout>
      <PageHeader contained>
        <PageTitle>API Usage</PageTitle>
      </PageHeader>
      <PageContent contained>
        <KuUsageHistory />
      </PageContent>
    </WorkspacesLayout>
  )
}
