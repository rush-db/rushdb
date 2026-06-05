/**
 * Analytics utility for the RushDB dashboard.
 *
 * All functions are cloud-only — they are no-ops when the platform is running
 * in self-hosted mode.  The `selfHosted` flag is read from the React Query
 * cache so these helpers can be called from mutation `onSuccess` callbacks
 * (outside of React component context).
 *
 * Replace GTM-XXXXX with the real container ID once it is created in GTM UI.
 */

import { queryClient } from '~/lib/queryClient'
import { queryKeys } from '~/lib/queryKeys'

// ── helpers ──────────────────────────────────────────────────────────────────

type PlatformSettings = { selfHosted: boolean }

function isSelfHosted(): boolean {
  const settings = queryClient.getQueryData<PlatformSettings>(queryKeys.settings())
  return settings?.selfHosted ?? false
}

function push(event: string, params: Record<string, unknown> = {}) {
  if (isSelfHosted()) return
  window.dataLayer = window.dataLayer || []
  window.dataLayer.push({ event, ...params })
}

// ── events ────────────────────────────────────────────────────────────────────

/** Fired after the user confirms their e-mail address (funnel Stage 4). */
export function trackSignupComplete(method: 'email' | 'google' | 'github' = 'email') {
  push('signup_complete', { signup_method: method })
}

/** Fired when the first (or any subsequent) project is created (Stage 5). */
export function trackProjectCreated(options: { isFirstProject?: boolean } = {}) {
  push('project_created', { is_first_project: options.isFirstProject ?? false })
}

/** Fired when an API key is generated (initial key created with a new project,
 *  or a key manually created on the tokens page). */
export function trackApiKeyGenerated(options: { isInitialKey?: boolean } = {}) {
  push('api_key_generated', { is_initial_key: options.isInitialKey ?? false })
}

/** Fired when the user clicks a paid-plan checkout button. */
export function trackUpgradeClicked(options: { currentPlan?: string; targetPlan?: string } = {}) {
  push('upgrade_clicked', {
    current_plan: options.currentPlan ?? 'free',
    target_plan: options.targetPlan ?? ''
  })
}

/** Fired when a plan-selection redirect completes successfully (Stripe session
 *  created and user redirected). */
export function trackPlanSelected(planName: string, billingPeriod: 'monthly' | 'annual') {
  push('plan_selected', { plan_name: planName, billing_period: billingPeriod })
}
