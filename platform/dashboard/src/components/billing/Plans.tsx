import { useEffect } from 'react'

import { useAvailablePlans, useCurrentWorkspacePlan } from '~/features/billing/hooks/useBillingHooks'
import { isFreePlan } from '~/features/billing/utils.ts'
import { FREE_PLAN } from '~/features/billing/constants.ts'
import { PlanCard } from '~/components/billing/PlanCard.tsx'
import { api } from '~/lib/api'
import { Link } from '~/elements/Link'
import { Message } from '~/elements/Message'
import type { DisplayPlan } from '~/features/billing/types'

export function Plans({ intendedPlan }: { intendedPlan?: string } = {}) {
  const plans = useAvailablePlans()
  const { currentPlan } = useCurrentWorkspacePlan()
  const paidUser = currentPlan && !isFreePlan(currentPlan)

  useEffect(() => {
    if (intendedPlan) {
      document
        .getElementById(`billing-plan-${intendedPlan}`)
        ?.scrollIntoView({ behavior: 'smooth', block: 'center' })
    }
  }, [intendedPlan])

  // 1 card: scale is active
  // 2 cards: pro is active + scale as upgrade
  // 3 cards: free + pro + scale
  let visiblePlans: DisplayPlan[]
  if (currentPlan?.id === 'scale') {
    visiblePlans = plans.filter((p) => p.id === 'scale')
  } else if (currentPlan?.id === 'pro') {
    visiblePlans = plans.filter((p) => p.id === 'pro' || p.id === 'scale')
  } else {
    visiblePlans = [FREE_PLAN, ...plans.filter((p) => p.id === 'pro' || p.id === 'scale')]
  }

  const gridCols =
    visiblePlans.length === 1 ? 'grid-cols-1 max-w-sm'
    : visiblePlans.length === 2 ? 'grid-cols-1 sm:grid-cols-2'
    : 'grid-cols-1 sm:grid-cols-2 xl:grid-cols-3'

  return (
    <>
      <div>
        {paidUser && (
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
      <div className={`grid gap-5 ${gridCols}`}>
        {visiblePlans.map((plan) => (
          <PlanCard
            active={plan.id === currentPlan?.id}
            highlighted={intendedPlan ? plan.id === intendedPlan : plan.id === 'pro'}
            id={`billing-plan-${plan.id}`}
            key={plan.id}
            plan={plan}
            perProject={plan.perProject}
          />
        ))}
      </div>
    </>
  )
}
