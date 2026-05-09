import { useMemo } from 'react'
import { useQuery, useMutation } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'
import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'
import { $currentWorkspaceId } from '~/features/workspaces/stores/current'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { useCurrentWorkspaceQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { FREE_PLAN } from '~/features/billing/constants'
import { isFreePlan } from '~/features/billing/utils'
import {
  pricingDataQueryOptions,
  workspaceUsageQueryOptions,
  kuHistoryQueryOptions
} from '../queries/billingQueries'
import type { DisplayPlan } from '../types'

export const usePricingDataQuery = () => {
  const { data: settings } = usePlatformSettings()
  return useQuery(pricingDataQueryOptions(settings?.selfHosted))
}

export const useWorkspaceUsageQuery = () => {
  const workspaceId = useStore($currentWorkspaceId)
  const { data: settings } = usePlatformSettings()
  return useQuery(workspaceUsageQueryOptions({ workspaceId, selfHosted: settings?.selfHosted }))
}

export const useKuHistoryQuery = (params: {
  limit?: number
  before?: string
  since?: string
  projectId?: string | null
  operation?: string | null
}) => {
  const workspaceId = useStore($currentWorkspaceId)
  const { data: settings } = usePlatformSettings()
  return useQuery(kuHistoryQueryOptions({ workspaceId, selfHosted: settings?.selfHosted, ...params }))
}

export const useAvailablePlans = () => {
  const { data: billingData } = usePricingDataQuery()
  return useMemo<Array<DisplayPlan>>(() => {
    if (!billingData) return []
    const plans: DisplayPlan[] = [
      {
        id: 'pro' as const,
        name: 'Pro',
        kuIncluded: billingData.pro.kuIncluded ?? 10_000_000,
        monthlyPriceId: billingData.pro.monthly.priceId,
        yearlyPriceId: billingData.pro.annual.priceId,
        monthlyPrice: billingData.pro.monthly.amount,
        yearlyPrice: billingData.pro.annual.amount
      }
    ]
    if (billingData.scale) {
      plans.push({
        id: 'scale' as const,
        name: 'Scale',
        kuIncluded: null,
        monthlyPriceId: billingData.scale.monthly.priceId,
        yearlyPriceId: billingData.scale.annual.priceId,
        monthlyPrice: billingData.scale.monthly.amount,
        yearlyPrice: billingData.scale.annual.amount
      })
    }
    return plans
  }, [billingData])
}

export const useCurrentWorkspacePlan = () => {
  const { data: workspace, isPending: loading } = useCurrentWorkspaceQuery()
  const availablePlans = useAvailablePlans()

  return useMemo(() => {
    if (loading) return { loading: true as const }
    const planId = workspace?.planId
    if (!planId) return { loading: false as const, currentPlan: FREE_PLAN }
    if (isFreePlan(planId)) return { loading: false as const, currentPlan: FREE_PLAN }
    const currentPlan = availablePlans.find((p) => p.id === planId)
    return { loading: false as const, currentPlan: currentPlan ?? FREE_PLAN }
  }, [workspace, loading, availablePlans])
}

export const useCheckoutMutation = () => {
  const { data: settings } = usePlatformSettings()
  const { refetch: refetchWorkspace } = useCurrentWorkspaceQuery()
  return useMutation({
    mutationFn: async (body: Parameters<typeof api.billing.createSession>[0]) => {
      if (!settings?.selfHosted) {
        const stripeKey = import.meta.env.VITE_STRIPE_PUBLIC_KEY
        if (!stripeKey) {
          throw new Error('Stripe is not configured. Set VITE_STRIPE_PUBLIC_KEY in your environment.')
        }
        const { loadStripe } = await import('@stripe/stripe-js')
        const stripe = await loadStripe(stripeKey)
        if (!stripe) throw new Error('Stripe failed to initialize')
        const session = await api.billing.createSession(body)
        const { error } = await stripe.redirectToCheckout({ sessionId: session.id })
        if (error) throw error
      }
    },
    onSuccess() {
      refetchWorkspace()
    }
  })
}
