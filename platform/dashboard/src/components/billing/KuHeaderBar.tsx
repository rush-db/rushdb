import { useStore } from '@nanostores/react'
import { $user } from '~/features/auth/stores/user'
import { useWorkspaceUsageQuery } from '~/features/billing/hooks/useBillingHooks'
import { cn } from '~/lib/utils'
import { getRoutePath } from '~/lib/router'

function formatKu(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n % 1_000_000 === 0 ? 0 : 1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(n % 1_000 === 0 ? 0 : 1)}K`
  return n.toLocaleString()
}

export function KuHeaderBar() {
  const currentUser = useStore($user)
  const { data: usage } = useWorkspaceUsageQuery()

  const isOwner = currentUser.currentScope?.role === 'owner'

  // selfHosted guard is baked into $workspaceUsage (returns undefined when selfHosted)
  if (!isOwner || !usage) return null

  // Scale / enterprise: just show consumed KU
  if (usage.billingModel === 'usage' || (usage.kuIncluded === null && usage.billingModel !== 'fixed')) {
    return (
      <a
        href={getRoutePath('workspaceApiUsage')}
        className="text-content3 hover:text-content hidden items-center gap-1.5 text-xs transition-colors sm:flex"
      >
        <span className="font-mono">{formatKu(usage.kuConsumed)} KU</span>
        <span className="capitalize opacity-60">{usage.plan}</span>
      </a>
    )
  }

  // Free / Pro: compact progress bar
  const cap = usage.kuIncluded!
  const pct = Math.min((usage.kuConsumed / cap) * 100, 100)
  const isOver = usage.kuConsumed > cap

  const barColor =
    isOver ? 'bg-danger'
    : pct >= 90 ? 'bg-warning'
    : pct >= 70 ? 'bg-warning/70'
    : 'bg-accent'

  const textColor =
    isOver ? 'text-danger'
    : pct >= 90 ? 'text-warning'
    : 'text-content3'

  return (
    <a
      href={getRoutePath('workspaceApiUsage')}
      className="hidden min-w-[120px] flex-col gap-1 sm:flex"
      title={`${formatKu(usage.kuConsumed)} / ${formatKu(cap)} KU (${pct.toFixed(1)}%)`}
    >
      <div className="flex items-center justify-between gap-2">
        <span className={cn('font-mono text-xs font-medium', textColor)}>
          {formatKu(usage.kuConsumed)}&thinsp;/&thinsp;{formatKu(cap)} KU
        </span>
        <span className="text-content3 text-xs opacity-60">{pct.toFixed(0)}%</span>
      </div>
      <div className="bg-secondary h-1 w-full overflow-hidden rounded-full">
        <div className={cn('h-full rounded-full transition-all', barColor)} style={{ width: `${pct}%` }} />
      </div>
    </a>
  )
}
