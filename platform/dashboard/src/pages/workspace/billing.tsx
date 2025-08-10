import { useStore } from '@nanostores/react'

import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { $pricingData } from '~/features/billing/stores/plans'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'

import { SelectPeriod } from '~/components/billing/SelectPeriod.tsx'
import { Plans } from '~/components/billing/Plans.tsx'
import { PricingCalculator } from '~/components/billing/PricingCalculator.tsx'

export function WorkspaceBillingPage() {
  const { loading } = useStore($pricingData)

  return (
    <WorkspacesLayout>
      <PageHeader contained>
        <PageTitle>Upgrade Plan</PageTitle>
        <SelectPeriod className="mx-auto" />
      </PageHeader>
      <PageContent className="gap-5" contained>
        <Plans />
        {loading ? null : (
          <div className="mb-8">
            <PricingCalculator />
          </div>
        )}
      </PageContent>
    </WorkspacesLayout>
  )
}
