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

type IncomingPlanData = {
  amount: number
  priceId: string
  productId: string
}

export type PlansData = {
  pro: {
    monthly: { amount: number; priceId: string; productId: string }
    annual: { amount: number; priceId: string; productId: string }
  }
  team: TieredPricingData
}
