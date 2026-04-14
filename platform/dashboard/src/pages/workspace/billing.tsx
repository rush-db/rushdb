import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'

import { SelectPeriod } from '~/components/billing/SelectPeriod.tsx'
import { Plans } from '~/components/billing/Plans.tsx'

export function WorkspaceBillingPage() {
  const intendedPlan = new URLSearchParams(window.location.search).get('plan') ?? undefined

  return (
    <WorkspacesLayout>
      <PageHeader contained className="justify-between">
        <PageTitle>Billing</PageTitle>
        <SelectPeriod />
      </PageHeader>
      <PageContent className="gap-5" contained>
        <Plans intendedPlan={intendedPlan} />
      </PageContent>
    </WorkspacesLayout>
  )
}
