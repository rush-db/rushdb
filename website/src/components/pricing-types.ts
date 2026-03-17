type PlanPricePoint = {
  amount: number
  priceId: string
  productId: string
}

export type KuPlanData = {
  monthly: PlanPricePoint
  annual: PlanPricePoint
  /** KU included in base price (null = unlimited) */
  kuIncluded: number | null
  /** Price per KU beyond included allowance, in dollars */
  overagePerKu?: number
  /** For usage-based plans: price per KU consumed, in dollars */
  perKuRate?: number
}

export type BillingData = {
  pro: KuPlanData
  scale?: KuPlanData
}
