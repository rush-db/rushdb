import { PRODUCT_PLAN_MAP } from '@/dashboard/billing/stripe/interfaces/stripe.constans'

export function getPlanKeyByPriceId(targetId: string): string | null {
  const currentPlanKey = Object.keys(PRODUCT_PLAN_MAP).find((planKey) =>
    Object.values(PRODUCT_PLAN_MAP[planKey]).some((period) => period.priceId === targetId)
  )

  return currentPlanKey || null
}

export function hasPlanInPalette(key: string) {
  return key in PRODUCT_PLAN_MAP
}
