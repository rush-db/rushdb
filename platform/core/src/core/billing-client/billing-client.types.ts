/**
 * Response from billing service check-limits endpoint.
 */
export interface CheckLimitsResponse {
  allowed: boolean
  reason?: string
  usage: {
    kuConsumed: number
    kuLimit: number | null
    plan: string
    remaining: number | null
    willIncurOverage?: boolean
    overageAmount?: number
  }
  limits: {
    projectLimit: number | null
    userLimit: number | null
    projectCount?: number
    userCount?: number
  }
}

/**
 * KU usage summary returned by GET /api/customers/:workspaceId/usage.
 */
export interface UsageResponse {
  plan: string
  kuConsumed: number
  /** KU cap for this period (null = unlimited) */
  kuLimit: number | null
  /** KU included in base price before overage kicks in */
  kuIncluded: number | null
  /** KU remaining before hitting limit or overage (null = unlimited) */
  remaining: number | null
  /** Billing model for current plan */
  billingModel: 'fixed' | 'overage' | 'usage'
  /** Start of current billing period (ISO 8601) */
  billingPeriodStart: string
}

/**
 * Customer record from billing service.
 */
export interface Customer {
  workspaceId: string
  stripeCustomerId?: string
  plan: 'free' | 'pro' | 'scale' | 'enterprise'
  kuLimit: number | null
  kuConsumed: number
  projectLimit: number | null
  userLimit: number | null
  billingPeriodStart: string
  subscriptionId?: string
  subscriptionStatus?: 'active' | 'canceled' | 'past_due' | 'trialing'
  createdAt: string
  updatedAt: string
  metadata?: Record<string, unknown>
}

export interface BillingInquiryPayload {
  contactEmail: string
  requesterEmail?: string
  workspaceId: string
  workspaceName?: string
  currentPlan?: string
  message?: string
}
