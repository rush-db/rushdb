import { TPlan } from '@/dashboard/billing/stripe/interfaces/stripe.types'

export function getPlanKeyByPriceId(targetId: string, planMap: TPlan): string | null {
  const currentPlanKey = Object.keys(planMap).find((planKey) =>
    Object.values<TPlan[keyof TPlan][keyof TPlan[keyof TPlan]]>(planMap[planKey]).some((period) => {
      const price = period.priceId
      return price === targetId
    })
  )

  return currentPlanKey || null
}
