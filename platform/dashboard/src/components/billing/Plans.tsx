import { useAvailablePlans, useCurrentWorkspacePlan } from '~/features/billing/hooks/useBillingHooks'
import { isFreePlan } from '~/features/billing/utils.ts'
import { FREE_PLAN } from '~/features/billing/constants.ts'
import { cn, range } from '~/lib/utils.ts'
import { Skeleton } from '~/elements/Skeleton.tsx'
import { NothingFound } from '~/elements/NothingFound.tsx'
import { PlanCard } from '~/components/billing/PlanCard.tsx'
import { api } from '~/lib/api'
import { Link } from '~/elements/Link'
import { Message } from '~/elements/Message'

export function Plans() {
  const plans = useAvailablePlans()
  const { currentPlan } = useCurrentWorkspacePlan()
  const paidUser = currentPlan && !isFreePlan(currentPlan)

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
      <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3">
        {!paidUser && <PlanCard active={true} className="order-last sm:order-first" plan={FREE_PLAN} />}

        {plans?.map((plan) => (
          <PlanCard
            active={plan.id === currentPlan?.id}
            className={cn(plan.id === currentPlan?.id ? 'lg:col-span-2' : 'lg:col-span-1')}
            key={plan.id}
            plan={plan}
            perProject={plan.perProject}
          />
        ))}

        {!plans &&
          range(1).map((index) => (
            <Skeleton className="lg:col-span-2" enabled key={index}>
              <PlanCard active={false} plan={FREE_PLAN} />
            </Skeleton>
          ))}

        {!plans && <NothingFound />}
      </div>
    </>
  )
}
