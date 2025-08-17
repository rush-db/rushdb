import { useStore } from '@nanostores/react'

import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import {
  $currentProjectPlan,
  $currentWorkspacePlan,
  $paidProject,
  $paidWorkspace,
  $pricingData
} from '~/features/billing/stores/plans'

import { SelectPeriod } from '~/components/billing/SelectPeriod.tsx'
import { PricingCalculator } from '~/components/billing/PricingCalculator.tsx'
import { CheckoutButton } from '~/components/billing/CheckoutButton.tsx'
import { SparklesIcon } from 'lucide-react'
import { api } from '~/lib/api.ts'
import { Link } from '~/elements/Link.tsx'
import { Message } from '~/elements/Message'

export function ProjectBillingPage() {
  const { loading } = useStore($pricingData)
  const { currentPlan, validTill, isSubscriptionCancelled } = useStore($currentProjectPlan)

  const paidProject = useStore($paidProject)

  return (
    <>
      <PageHeader contained>
        <PageTitle>Upgrade Plan</PageTitle>
        <SelectPeriod className="items-end justify-end" />
      </PageHeader>
      <PageContent className="gap-5" contained>
        <div>
          {paidProject && (
            <Message variant="info" size="medium" className="mb-5 w-fit !p-4">
              You're on a paid plan — manage your subscription in the billing portal:
              <form
                onSubmit={async (event) => {
                  event.preventDefault()
                  const { redirectUrl } = await api.billing.createPortalSession({
                    returnUrl: window.location.href
                  })
                  if (redirectUrl) {
                    window.location.replace(redirectUrl)
                  }
                }}
              >
                <Link as="button" type="submit">
                  Manage Subscription
                </Link>
              </form>
            </Message>
          )}
        </div>

        {loading ? null : (
          <div className="mb-8">
            <PricingCalculator
              cta={(priceId) => (
                <CheckoutButton
                  variant="accent"
                  priceId={priceId}
                  className="mt-8 w-full min-w-48 justify-center font-semibold"
                >
                  Upgrade Plan
                  <SparklesIcon />
                </CheckoutButton>
              )}
            />
          </div>
        )}
      </PageContent>
    </>
  )
}
