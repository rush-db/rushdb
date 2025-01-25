export type PaidPlanId = 'pro'
export type FreePlanId = 'free'
export type PlanId = FreePlanId | PaidPlanId

export type PlanPeriod = 'annual' | 'month'

export type IncomingBillingData = {
  pro: {
    month: {
      amount: number
      priceId: string
      productId: string
    }
    annual: {
      amount: number
      priceId: string
      productId: string
    }
  }
}

export type PaidPlan = {
  id: PaidPlanId
  name: string
  monthlyPrice: number
  yearlyPrice: number
}

export type FreePlan = {
  id: FreePlanId
  name: string
}

export type Plan = FreePlan | PaidPlan
