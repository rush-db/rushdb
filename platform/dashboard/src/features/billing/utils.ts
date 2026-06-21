export const isFreePlan = (plan: any) => plan.id === 'free'

export async function sleep(delayMs: number) {
  return new Promise((resolve) => setTimeout(resolve, delayMs))
}
