export type PaidStartPlanId = 'start'
export type PaidPlanId = 'pro'
export type FreePlanId = 'free'
export type PlanId = FreePlanId | PaidStartPlanId | PaidPlanId

export type PlanPeriod = 'annual' | 'month'

type IncomingPlanData = {
  amount: number
  priceId: string
  productId: string
}

type InstanceSpecs = {
  ram: number
  cpu: number
  disk: number
}

type InstancePricing = {
  onDemand: IncomingPlanData
  reserved: IncomingPlanData
}

type TieredPricingData = Array<{ tier: string } & InstancePricing & InstanceSpecs>

export type BillingData = {
  pro: {
    monthly: { amount: number; priceId: string; productId: string }
    annual: { amount: number; priceId: string; productId: string }
  }
  team: TieredPricingData
}
