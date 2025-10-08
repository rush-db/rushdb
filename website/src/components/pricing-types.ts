type InstanceSpecs = {
  ram: number
  cpu: number
  disk: number
}

type InstancePricing = {
  onDemand: IncomingPlanData
  reserved: IncomingPlanData
}

export type TieredPricingData = Array<{ tier: string } & InstancePricing & InstanceSpecs>

type IncomingPlanData = {
  amount: number
  priceId: string
  productId: string
}

export type BillingData = {
  pro: {
    monthly: { amount: number; priceId: string; productId: string }
    annual: { amount: number; priceId: string; productId: string }
  }
  team: TieredPricingData
}
