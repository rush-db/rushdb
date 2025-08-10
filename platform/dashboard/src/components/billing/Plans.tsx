import { useStore } from '@nanostores/react'
import { $availablePlans, $currentWorkspacePlan, $paidWorkspace } from '~/features/billing/stores/plans.ts'
import { FREE_PLAN } from '~/features/billing/constants.ts'
import { cn, range } from '~/lib/utils.ts'
import { Skeleton } from '~/elements/Skeleton.tsx'
import { NothingFound } from '~/elements/NothingFound.tsx'
import React from 'react'
import { PlanCard } from '~/components/billing/PlanCard.tsx'

export function Plans() {
  const plans = useStore($availablePlans)
  const { currentPlan } = useStore($currentWorkspacePlan)
  const paidUser = useStore($paidWorkspace)

  return (
    <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3">
      {!paidUser && <PlanCard active={true} className="order-last sm:order-first" plan={FREE_PLAN} />}

      {plans?.map((plan) => (
        <PlanCard
          active={plan.id === currentPlan?.id}
          className={cn(plan.id === currentPlan?.id ? 'lg:col-span-2' : 'lg:col-span-1')}
          key={plan.id}
          plan={plan}
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
  )
}
