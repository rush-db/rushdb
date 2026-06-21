import { useStore } from '@nanostores/react'
import { ArrowUpRight, Sparkles } from 'lucide-react'

import { $user } from '~/features/auth/stores/user'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { useWorkspaceUsageQuery } from '~/features/billing/hooks/useBillingHooks'
import { useCurrentWorkspaceQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { formatIsoToLocal } from '~/lib/formatters'
import { cn } from '~/lib/utils'
import { getRoutePath } from '~/lib/router'

function formatKu(n: number | null | undefined): string {
  if (n == null) return '—'
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n % 1_000_000 === 0 ? 0 : 1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(n % 1_000 === 0 ? 0 : 1)}K`
  return n.toLocaleString()
}

/**
 * Compact Knowledge Units usage meter for the sidebar footer. Shows the current
 * billing-period consumption against the plan cap, and nudges free users to upgrade
 * with a light, value-forward CTA. Hidden for self-hosted instances and non-owners.
 */
export function KuSidebarMeter() {
  const currentUser = useStore($user)
  const { data: platformSettings } = usePlatformSettings()
  const { data: usage } = useWorkspaceUsageQuery()
  const { data: workspace } = useCurrentWorkspaceQuery()

  const isOwner = currentUser.currentScope?.role === 'owner'

  if (platformSettings?.selfHosted || !isOwner || !usage || usage.kuConsumed == null) return null

  const isFree = (usage.plan ?? '').toLowerCase() === 'free'
  const isCancelled = !!workspace?.isSubscriptionCancelled
  const validTill = workspace?.validTill
  const billingHref = getRoutePath('workspaceBilling')

  // Subscription state, baked into the meter block (replaces the old badge that
  // sat next to the user avatar).
  const subscriptionState =
    isFree ?
      <a
        className="border-accent/30 bg-accent/5 hover:bg-accent/10 -mx-1 -mb-1 mt-0.5 flex items-center gap-2 rounded-md border px-2.5 py-2 transition"
        href={billingHref}
      >
        <Sparkles className="text-accent h-4 w-4 shrink-0" />
        <span className="text-content2 text-xs leading-snug">
          <span className="text-content font-medium">Upgrade</span> for more Knowledge Units and higher
          limits.
        </span>
      </a>
    : isCancelled && validTill ?
      <a className="text-content2 hover:text-content text-xs transition" href={billingHref}>
        Subscription ends {formatIsoToLocal(validTill)}
      </a>
    : <a className="text-content3 hover:text-content text-xs transition" href={billingHref}>
        Active subscription
      </a>

  // Usage-based plans (Scale / Enterprise): no fixed cap to meter, just surface consumption.
  if (usage.billingModel === 'usage' || (usage.kuIncluded === null && usage.billingModel !== 'fixed')) {
    return (
      <div className="border-content/20 bg-fill flex flex-col gap-2 rounded-md border p-4">
        <a className="group flex flex-col gap-1.5" href={getRoutePath('workspaceApiUsage')}>
          <div className="flex items-center justify-between gap-2">
            <span className="text-content2 text-xs font-medium uppercase tracking-wide">Knowledge Units</span>
            <span className="text-content3 text-xs capitalize">{usage.plan}</span>
          </div>
          <span className="text-content font-mono text-sm font-medium">{formatKu(usage.kuConsumed)} KU</span>
          <span className="text-content3 inline-flex items-center gap-1 text-xs">
            View usage <ArrowUpRight className="h-3 w-3" />
          </span>
        </a>
        {subscriptionState}
      </div>
    )
  }

  // Fixed-cap plans (Free / Start / Pro): metered progress bar.
  const cap = usage.kuIncluded!
  const pct = cap > 0 ? Math.min((usage.kuConsumed / cap) * 100, 100) : 0
  const isOver = usage.kuConsumed > cap

  const barColor =
    isOver ? 'bg-danger'
    : pct >= 90 ? 'bg-warning'
    : pct >= 70 ? 'bg-warning/70'
    : 'bg-accent'

  const textColor =
    isOver ? 'text-danger'
    : pct >= 90 ? 'text-warning'
    : 'text-content'

  return (
    <div className="border-content/20 bg-fill flex flex-col gap-2.5 rounded-md border p-4">
      <a
        className="group flex flex-col gap-2"
        href={getRoutePath('workspaceApiUsage')}
        title={`${formatKu(usage.kuConsumed)} / ${formatKu(cap)} KU (${pct.toFixed(1)}%)`}
      >
        <div className="flex items-center justify-between gap-2">
          <span className="text-content2 text-xs font-medium uppercase tracking-wide">Knowledge Units</span>
          <span className="text-content3 text-xs capitalize">{usage.plan}</span>
        </div>
        <div className="flex items-baseline justify-between gap-2">
          <span className={cn('font-mono text-sm font-medium', textColor)}>
            {formatKu(usage.kuConsumed)}&thinsp;/&thinsp;{formatKu(cap)}
          </span>
          <span className="text-content3 text-xs">{pct.toFixed(0)}%</span>
        </div>
        <div className="bg-secondary h-1.5 w-full overflow-hidden rounded-full">
          <div className={cn('h-full rounded-full transition-all', barColor)} style={{ width: `${pct}%` }} />
        </div>
      </a>

      {subscriptionState}
    </div>
  )
}
