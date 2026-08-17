/**
 * Plan → entitlement resolution for the synx connector catalog.
 *
 * Synx owns the provider union and each connector's minimum tier. Given the
 * requesting workspace's plan id, this returns the connectors the workspace
 * may use plus the visible-but-locked ones. Core only forwards the plan id —
 * it never hardcodes connector references.
 *
 * Plan ordering: the `free` tier is lowest; any plan id not recognised is
 * treated as `paid` (a non-free workspace) so it never loses access it had.
 */

import type { SynxConnectorTierV1, SynxConnectorV1, SynxUnavailableConnectorV1 } from './connectors'

/** Map a workspace plan id onto a synx entitlement tier. */
export function planIdToTier(planId: string | undefined | null): SynxConnectorTierV1 {
  if (!planId || planId === 'free') return 'free'
  if (planId === 'enterprise') return 'top_tier'
  // `starter`/`pro`/`scale`/`business` etc. are all non-free here.
  return 'paid'
}

const TIER_RANK: Record<SynxConnectorTierV1, number> = { free: 0, paid: 1, top_tier: 2 }

/** True when `plan` satisfies a connector's `entitlement` tier. */
export function tierSatisfies(required: SynxConnectorTierV1, plan: SynxConnectorTierV1): boolean {
  return TIER_RANK[plan] >= TIER_RANK[required]
}

export interface SynxCatalogByPlanV1 {
  connectors: SynxConnectorV1[]
  unavailable: SynxUnavailableConnectorV1[]
}

/**
 * Partition a full catalog for a workspace plan. `entitledOnly` includes every
 * provider (entitled ones in `connectors`, the rest in `unavailable`); pass
 * `true` to drop locked providers entirely (used by the worker so it never
 * learns about providers its workspace cannot use).
 */
export function catalogForPlan(
  catalog: SynxConnectorV1[],
  planId: string | undefined | null,
  entitledOnly = false
): SynxCatalogByPlanV1 {
  const plan = planIdToTier(planId)
  const connectors: SynxConnectorV1[] = []
  const unavailable: SynxUnavailableConnectorV1[] = []

  for (const connector of catalog) {
    const required = connector.entitlement ?? 'free'
    if (tierSatisfies(required, plan)) {
      connectors.push(connector)
    } else if (!entitledOnly) {
      unavailable.push({
        id: connector.id,
        name: connector.name,
        requiredTier: required,
        reason:
          connector.unavailableReason ??
          `Available on the ${required === 'top_tier' ? 'top-tier' : required} plan`,
        icon: connector.icon
      })
    }
  }
  return { connectors, unavailable }
}
