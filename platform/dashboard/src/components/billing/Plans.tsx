import { useState, useEffect } from 'react'

import { useAvailablePlans, useCurrentWorkspacePlan } from '~/features/billing/hooks/useBillingHooks'
import { isFreePlan } from '~/features/billing/utils.ts'
import { CUSTOM_PLAN, FREE_PLAN } from '~/features/billing/constants.ts'
import { PlanCard } from '~/components/billing/PlanCard.tsx'
import { api } from '~/lib/api'
import { Link } from '~/elements/Link'
import { Message } from '~/elements/Message'
import { useCurrentUserQuery } from '~/features/auth/hooks/useAuthQueries'
import { useCurrentWorkspaceQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { CustomPlanInquiryModal } from '~/components/billing/CustomPlanInquiryModal'
import type { DisplayPlan } from '~/features/billing/types'

export function Plans({ intendedPlan }: { intendedPlan?: string } = {}) {
  const plans = useAvailablePlans()
  const { currentPlan } = useCurrentWorkspacePlan()
  const { data: currentUser } = useCurrentUserQuery()
  const { data: workspace } = useCurrentWorkspaceQuery()
  const paidUser = currentPlan && !isFreePlan(currentPlan)
  const [customInquiryOpen, setCustomInquiryOpen] = useState(false)

  useEffect(() => {
    if (intendedPlan) {
      document
        .getElementById(`billing-plan-${intendedPlan}`)
        ?.scrollIntoView({ behavior: 'smooth', block: 'center' })
    }
  }, [intendedPlan])

  const visiblePlans: DisplayPlan[] = [...(!paidUser ? [FREE_PLAN] : []), ...plans, CUSTOM_PLAN]

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
      <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 xl:grid-cols-4">
        {visiblePlans.map((plan) => (
          <PlanCard
            active={plan.id === currentPlan?.id}
            className={plan.id === 'free' ? 'order-last sm:order-first' : undefined}
            highlighted={intendedPlan ? plan.id === intendedPlan : plan.id === 'pro'}
            id={`billing-plan-${plan.id}`}
            key={plan.id}
            onAction={plan.id === CUSTOM_PLAN.id ? () => setCustomInquiryOpen(true) : undefined}
            plan={plan}
            perProject={plan.perProject}
          />
        ))}
      </div>

      <CustomPlanInquiryModal
        currentPlan={currentPlan?.id ?? FREE_PLAN.id}
        defaultEmail={currentUser?.login}
        onOpenChange={setCustomInquiryOpen}
        open={customInquiryOpen}
        workspaceName={workspace?.name}
      />
    </>
  )
}
