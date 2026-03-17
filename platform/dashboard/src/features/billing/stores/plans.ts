import { persistentAtom } from '@nanostores/persistent'
import { computed } from 'nanostores'

import type { PlanPeriod } from '~/features/billing/types'

import { FREE_PLAN } from '~/features/billing/constants'
import { isFreePlan, sleep } from '~/features/billing/utils'
import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace'
import { $currentProject } from '~/features/projects/stores/current-project'
import { createAsyncStore } from '~/lib/fetcher.ts'
import { api } from '~/lib/api.ts'
import { $platformSettings } from '~/features/auth/stores/settings.ts'

type AvailablePlan = {
  id: string
  name: string
  kuIncluded: number | null
  monthlyPriceId: string
  yearlyPriceId: string
  monthlyPrice: number
  yearlyPrice: number
  perProject?: boolean
}

export const $pricingData = createAsyncStore({
  key: '$pricingData',
  skip: () => Boolean($platformSettings.get().data?.selfHosted),
  async fetcher() {
    // @TODO: Fix this dirty hack
    await sleep(2000)
    return await api.billing.getBillingData()
  },
  mustHaveDeps: [$platformSettings]
})

export const $availablePlans = computed($pricingData, (billingData) => {
  if (!billingData.data) {
    return []
  }

  const plans: AvailablePlan[] = [
    {
      id: 'pro',
      name: 'Pro',
      kuIncluded: billingData.data.pro.kuIncluded ?? 10_000_000,
      monthlyPriceId: billingData.data.pro.monthly.priceId,
      yearlyPriceId: billingData.data.pro.annual.priceId,
      monthlyPrice: billingData.data.pro.monthly.amount,
      yearlyPrice: billingData.data.pro.annual.amount
    }
  ]

  if (billingData.data.scale) {
    plans.push({
      id: 'scale',
      name: 'Scale',
      kuIncluded: null,
      monthlyPriceId: billingData.data.scale.monthly.priceId,
      yearlyPriceId: billingData.data.scale.annual.priceId,
      monthlyPrice: billingData.data.scale.monthly.amount,
      yearlyPrice: billingData.data.scale.annual.amount
    })
  }

  return plans
})

export const $currentWorkspacePlan = computed([$availablePlans, $currentWorkspace], (plans, workspace) => {
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

export const $paidWorkspace = computed($currentWorkspacePlan, ({ currentPlan }) =>
  Boolean(currentPlan && !isFreePlan(currentPlan))
)

export const $currentPeriod = persistentAtom<PlanPeriod>('billing:period', 'annual')

// Project billing is deprecated - billing is workspace-level only
// These stores remain for backward compatibility but always return workspace plan
export const $currentProjectPlan = $currentWorkspacePlan

export const $paidProject = $paidWorkspace
