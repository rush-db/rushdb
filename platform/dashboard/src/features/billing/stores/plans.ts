import { persistentAtom } from '@nanostores/persistent'
import { computed } from 'nanostores'

import type { PaidPlan, PlanPeriod } from '~/features/billing/types'

import { FREE_PLAN } from '~/features/billing/constants'
import { isFreePlan } from '~/features/billing/utils'
import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace'
import { createAsyncStore } from '~/lib/fetcher.ts'
import { api } from '~/lib/api.ts'

export const $billingSettings = createAsyncStore({
  key: '$billingSettings',
  async fetcher() {
    return await api.billing.getBillingData()
  },
  deps: []
})

export const $availablePlans = computed($billingSettings, (billingData) => {
  if (!billingData.data) {
    return [] as PaidPlan[]
  }

  return [
    {
      id: 'start',
      name: 'RushDB Start',
      monthlyPrice: billingData.data.start.month.amount,
      yearlyPrice: billingData.data.start.annual.amount
    },
    {
      id: 'pro',
      name: 'RushDB Pro',
      monthlyPrice: billingData.data.pro.month.amount,
      yearlyPrice: billingData.data.pro.annual.amount
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
