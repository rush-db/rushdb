import type { FreePlan, Plan } from '~/features/billing/types'

export const isFreePlan = (plan: Pick<Plan, 'id'>): plan is FreePlan => plan.id === 'free'

export async function sleep(delayMs: number) {
  return new Promise((resolve) => setTimeout(resolve, delayMs))
}
