import type { FreePlan, PlanPeriod } from '~/features/billing/types'

export const FREE_PLAN: FreePlan = {
  id: 'free',
  name: 'Free'
}

export const PLAN_PERIODS: Array<{ name: string; value: PlanPeriod }> = [
  { value: 'month', name: 'monthly' },
  { value: 'annual', name: 'annual' }
]

export enum BillingErrorCodes {
  PaymentRequired = 402
}
