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

export type IncomingBillingData = Record<PaidStartPlanId | PaidPlanId, Record<PlanPeriod, IncomingPlanData>>

export type PaidPlan = {
  id: PaidPlanId | PaidStartPlanId
  name: string
  monthlyPrice: number
  yearlyPrice: number
}

export type FreePlan = {
  id: FreePlanId
  name: string
}

export type Plan = FreePlan | PaidPlan
