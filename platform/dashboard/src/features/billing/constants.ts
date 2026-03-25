export const FREE_PLAN = {
  id: 'free' as const,
  name: 'Free',
  kuIncluded: 100_000
}

export const CUSTOM_PLAN = {
  id: 'custom' as const,
  name: 'Custom',
  kuIncluded: null,
  inquiryOnly: true,
  ctaLabel: "Let's talk"
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
