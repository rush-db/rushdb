import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'

import { SelectPeriod } from '~/components/billing/SelectPeriod.tsx'
import { Plans } from '~/components/billing/Plans.tsx'
import { PricingComparison } from '~/components/billing/PricingComparison.tsx'
import { UsageV3Meter } from '~/components/billing/UsageV3Meter.tsx'

export function WorkspaceBillingPage() {
  const intendedPlan = new URLSearchParams(window.location.search).get('plan') ?? undefined

  return (
    <WorkspacesLayout>
      <PageHeader contained className="justify-between">
        <PageTitle>Billing</PageTitle>
        <SelectPeriod />
      </PageHeader>
      <PageContent className="gap-5" contained>
        <UsageV3Meter />
        <Plans intendedPlan={intendedPlan} />
        <PricingComparison className="mt-8" />
      </PageContent>
    </WorkspacesLayout>
  )
}
