import { persistentAtom } from '@nanostores/persistent'
import { atom, computed } from 'nanostores'

import type { PlanPeriod } from '~/features/billing/types'

import { FREE_PLAN } from '~/features/billing/constants'
import { isFreePlan, sleep } from '~/features/billing/utils'
import { $currentWorkspace } from '~/features/workspaces/stores/current-workspace'
import { createAsyncStore } from '~/lib/fetcher.ts'
import { api } from '~/lib/api.ts'
import { $platformSettings } from '~/features/auth/stores/settings.ts'
import { $currentProject } from '~/features/projects/stores/current-project.ts'

export const $pricingData = createAsyncStore({
  key: '$pricingData',
  async fetcher() {
    // @TODO: Fix this dirty hack
    await sleep(2000)
    if ($platformSettings.get().data?.selfHosted) {
      return
    }
    return await api.billing.getBillingData()
  },
  mustHaveDeps: [$platformSettings]
})

export const $currentTierIndex = atom<number>(0)

const $currentTier = computed([$pricingData, $currentTierIndex], (billingData, currentTierIndex) => {
  if (!billingData.data) {
    return null
  }

  return billingData.data.team[currentTierIndex].tier
})

export const $availablePlans = computed([$pricingData, $currentTier], (billingData, currentTier) => {
  if (!billingData.data) {
    return []
  }

  const teamTier = billingData.data.team.find((tier) => tier.tier === currentTier)

  return [
    {
      id: 'pro',
      name: 'Pro',
      monthlyPriceId: billingData.data.pro.monthly.priceId,
      yearlyPriceId: billingData.data.pro.annual.priceId,
      monthlyPrice: billingData.data.pro.monthly.amount,
      yearlyPrice: billingData.data.pro.annual.amount
    },
    {
      id: 'team',
      name: 'Team',
      // @TODO: Fix this dirty hack
      perProject: true,
      monthlyPriceId: teamTier?.onDemand.priceId,
      yearlyPriceId: teamTier?.reserved.priceId,
      monthlyPrice: teamTier?.onDemand.amount,
      yearlyPrice: teamTier?.reserved.amount
    }
  ]
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

export const $currentProjectPlan = computed([$availablePlans, $currentProject], (plans, project) => {
  const loading = project.loading

  if (loading) {
    return { loading: true as const }
  }

  const planId = project.data?.planId

  if (!planId) {
    return {
      loading: false as const,
      currentPlan: FREE_PLAN,
      validTill: project.data?.validTill,
      isSubscriptionCancelled: project.data?.isSubscriptionCancelled
    }
  }

  const foundPlan = plans.find((plan) => plan.id === planId)

  const currentPlan = foundPlan ?? FREE_PLAN

  return {
    loading: false as const,
    currentPlan,
    validTill: project.data?.validTill,
    isSubscriptionCancelled: project.data?.isSubscriptionCancelled
  }
})

export const $paidProject = computed($currentProjectPlan, ({ currentPlan }) =>
  Boolean(currentPlan && !isFreePlan(currentPlan))
)
