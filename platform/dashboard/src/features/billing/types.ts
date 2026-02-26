export type FreePlanId = 'free' | 'start'
export type PaidPlanId = 'pro' | 'scale'
export type EnterprisePlanId = 'enterprise'
export type PlanId = FreePlanId | PaidPlanId | EnterprisePlanId

export type PlanPeriod = 'annual' | 'month'

type IncomingPlanData = {
  amount: number
  priceId: string
  productId: string
}

type KuPlanData = {
  monthly: IncomingPlanData
  annual: IncomingPlanData
  /** Included KU per month (null = unlimited) */
  kuIncluded: number | null
  /** Price per KU above the included allowance (in dollars) */
  overagePerKu?: number
  /** For usage-based plans: price per KU consumed (in dollars) */
  perKuRate?: number
}

export type BillingData = {
  pro: KuPlanData
  scale?: KuPlanData
}
