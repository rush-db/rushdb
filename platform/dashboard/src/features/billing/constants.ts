export const FREE_PLAN = {
  id: 'free' as const,
  name: 'Free'
}

export const KU_PLAN_LABELS: Record<string, string> = {
  free: '100K KU / month',
  start: '100K KU / month',
  pro: '10M KU / month',
  scale: 'Usage-based',
  enterprise: 'Unlimited'
}

export enum BillingErrorCodes {
  PaymentRequired = 402
}
