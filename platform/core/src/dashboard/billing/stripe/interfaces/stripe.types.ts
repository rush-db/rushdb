export type PlanName = 'pro' | 'start'
export type PlanPeriod = 'month' | 'annual'

type TPlanPayload = {
  amount: number
  priceId: string
  productId: string
}

export type TPlan = Record<PlanName, Record<PlanPeriod, TPlanPayload>>
