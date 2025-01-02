import { persistentAtom } from '@nanostores/persistent'
import { computed } from 'nanostores'

import type { PaidPlan, Plan, PlanPeriod } from '~/features/billing/types'

import { FREE_PLAN } from '~/features/billing/constants'
import { isFreePlan } from '~/features/billing/utils'
import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace'
import { createAsyncStore } from '~/lib/fetcher'

export const PRODUCT_PLAN_MAP = {
  pro: {
    month: {
      monthPrice: 11,
      total: 11
    },
    annual: {
      monthPrice: 8.25,
      total: 99
    }
  }
}

export const $availablePlans = computed([], () => {
  return [
    {
      id: 'pro',
      name: 'RushDB Pro',
      price: 11,
      annualDiscount: 0.25
    }
  ] as PaidPlan[]
})

export const $currentPlan = computed([$availablePlans, $currentWorkspace], (plans, workspace) => {
  const loading = workspace.loading

  if (loading) {
    return { loading: true as const }
  }

  const planId = workspace.data?.planId

  if (!planId) {
    return {
      loading: false as const,
      currentPlan: FREE_PLAN,
      validTill: workspace.data?.validTill,
      isSubscriptionCancelled: workspace.data?.isSubscriptionCancelled
    }
  }

  const foundPlan = plans.find((plan) => plan.id === planId)

  const currentPlan = foundPlan ?? FREE_PLAN

  return {
    loading: false as const,
    currentPlan,
    validTill: workspace.data?.validTill,
    isSubscriptionCancelled: workspace.data?.isSubscriptionCancelled
  }
})

export const $paidUser = computed($currentPlan, ({ currentPlan }) =>
  Boolean(currentPlan && !isFreePlan(currentPlan))
)

export const $currentPeriod = persistentAtom<PlanPeriod>('billing:period', 'annual')
