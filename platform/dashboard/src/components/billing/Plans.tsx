import { useEffect } from 'react'

import {
  useAvailablePlans,
  useCurrentWorkspacePlan,
  usePricingDataQuery
} from '~/features/billing/hooks/useBillingHooks'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { isFreePlan } from '~/features/billing/utils.ts'
import { FREE_PLAN } from '~/features/billing/constants.ts'
import { PlanCard, PlanCardSkeleton } from '~/components/billing/PlanCard.tsx'
import { api } from '~/lib/api'
import { Link } from '~/elements/Link'
import { toast } from '~/elements/Toast'
import type { DisplayPlan } from '~/features/billing/types'

// Tier ordering used to decide whether a plan is an upgrade, the current plan, or
// a downgrade relative to the workspace's current plan.
const TIER_RANK: Record<string, number> = { free: 0, start: 1, pro: 2, scale: 3, enterprise: 4 }

export function Plans({ intendedPlan }: { intendedPlan?: string } = {}) {
  const plans = useAvailablePlans()
  const { data: settings, isPending: settingsPending } = usePlatformSettings()
  const { data: billingData, isLoading: pricingLoading } = usePricingDataQuery()
  const { currentPlan } = useCurrentWorkspacePlan()

  // Show skeletons until we know the tier (settings) and, for hosted workspaces,
  // until pricing has loaded — otherwise the grid briefly renders only the Free
  // card, which reads as "Free is your only option".
  const showSkeleton = settingsPending || (settings?.selfHosted === false && pricingLoading)
  const paidUser = currentPlan && !isFreePlan(currentPlan)
  const currentRank = TIER_RANK[currentPlan?.id ?? 'free'] ?? 0

  // Always show the full ladder (Free + every fetched paid plan) so the current
  // plan, upgrades, and downgrades are all visible — not just the current tier
  // and above. Benefits come from the billing service (single source of truth);
  // PlanBenefits falls back to a canonical copy when the service hasn't loaded.
  const ladder: DisplayPlan[] = [FREE_PLAN, ...plans]
  const allPlans: DisplayPlan[] = ladder.map((plan) => ({
    ...plan,
    benefits: plan.benefits ?? billingData?.benefits?.[plan.id]
  }))

  // Recommended upsell = the next paid tier directly above the current plan.
  const nextUpgrade = allPlans.find(
    (plan) => plan.id !== 'free' && (TIER_RANK[plan.id] ?? 0) > currentRank
  )?.id
  const recommendedId = intendedPlan ?? nextUpgrade

  useEffect(() => {
    if (intendedPlan) {
      document
        .getElementById(`billing-plan-${intendedPlan}`)
        ?.scrollIntoView({ behavior: 'smooth', block: 'center' })
    }
  }, [intendedPlan])

  const openBillingPortal = async () => {
    try {
      const { redirectUrl } = await api.billing.createPortalSession({
        returnUrl: window.location.href
      })
      if (redirectUrl) {
        window.location.replace(redirectUrl)
      }
    } catch {
      toast({
        title: "Couldn't open the billing portal",
        description:
          'We couldn’t find an active subscription to manage. Please refresh, or contact support if this keeps happening.',
        variant: 'danger'
      })
    }
  }

  const SKELETON_COUNT = 4
  const cardCount = showSkeleton ? SKELETON_COUNT : allPlans.length
  const gridCols =
    cardCount <= 1 ? 'grid-cols-1 max-w-sm'
    : cardCount === 2 ? 'grid-cols-1 sm:grid-cols-2'
    : cardCount === 3 ? 'grid-cols-1 sm:grid-cols-2 xl:grid-cols-3'
    : 'grid-cols-1 sm:grid-cols-2 xl:grid-cols-4'

  return (
    <>
      <div className="flex flex-wrap items-start justify-between gap-4">
        <p className="text-content2 max-w-2xl text-sm leading-6">
          RushDB pricing scales with <span className="text-content font-medium">Knowledge Units (KU)</span> —
          the single metric for the work your database does across reads, writes, and searches. Pick the plan
          with the headroom you need; you can change plans anytime and we prorate the difference.
        </p>
        {paidUser ?
          <div className="text-content2 shrink-0 text-sm">
            You're on <span className="text-content font-medium">{currentPlan?.name}</span>.{' '}
            <Link as="button" onClick={openBillingPortal} type="button">
              Manage subscription
            </Link>
          </div>
        : null}
      </div>

      <div className={`grid gap-5 ${gridCols}`}>
        {showSkeleton ?
          Array.from({ length: SKELETON_COUNT }).map((_, index) => <PlanCardSkeleton key={index} />)
        : allPlans.map((plan) => {
            const rank = TIER_RANK[plan.id] ?? 0
            const relation =
              rank === currentRank ? 'current'
              : rank > currentRank ? 'upgrade'
              : 'downgrade'

            return (
              <PlanCard
                id={`billing-plan-${plan.id}`}
                key={plan.id}
                onManage={openBillingPortal}
                perProject={plan.perProject}
                plan={plan}
                recommended={plan.id === recommendedId && relation === 'upgrade'}
                relation={relation}
              />
            )
          })
        }
      </div>
    </>
  )
}
