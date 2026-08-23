import { useStore } from '@nanostores/react'
import { Database, Sparkles } from 'lucide-react'

import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { useWorkspaceUsageV3Query } from '~/features/billing/hooks/useBillingHooks'

function formatCompact(value: number): string {
  if (value >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(1)}B`
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
  return value.toLocaleString(undefined, { maximumFractionDigits: 2 })
}

function UsageBar({
  label,
  icon,
  consumed,
  included
}: {
  label: string
  icon: 'facts' | 'credits'
  consumed: number
  included: number | null
}) {
  const pct = included !== null && included > 0 ? Math.min((consumed / included) * 100, 100) : 0
  const barColor =
    pct >= 90 ? '#ef4444'
    : pct >= 70 ? '#f59e0b'
    : 'hsl(72.96 82.69% 61.55%)'

  return (
    <div className="rounded-xl border bg-fill2 p-4">
      <div className="mb-2 flex items-center gap-2">
        {icon === 'facts' ?
          <Database className="h-4 w-4 text-accent" />
        : <Sparkles className="h-4 w-4 text-accent" />}
        <span className="text-xs font-medium tracking-wide text-content3 uppercase">{label}</span>
      </div>
      <p className="typography-2xl font-bold text-content">
        {formatCompact(consumed)}
        {included !== null ?
          <span className="text-sm font-normal text-content3"> / {formatCompact(included)}</span>
        : null}
      </p>
      {included !== null ?
        <div className="mt-3 h-2 w-full overflow-hidden rounded-full bg-fill3">
          <div
            className="h-full rounded-full transition-all"
            style={{
              width: `${included > 0 ? Math.min((consumed / included) * 100, 100) : 0}%`,
              backgroundColor: barColor
            }}
          />
        </div>
      : <p className="mt-1 text-xs text-content3">unlimited</p>}
    </div>
  )
}

/**
 * v3 usage meters (context facts / agent query credits) for the current
 * billing period. Renders nothing in self-hosted mode.
 */
export function UsageV3Meter() {
  const { data: settings } = usePlatformSettings()
  const { data: usage, isPending } = useWorkspaceUsageV3Query()

  if (settings?.selfHosted || isPending || !usage || usage.plan === 'self-hosted') {
    return null
  }

  return (
    <div>
      <div className="mb-3 flex items-center justify-between">
        <h2 className="typography-lg font-semibold text-content">Usage this period</h2>
        {usage.projectedOverageUsd > 0 ?
          <span className="text-sm text-content3">
            Projected overage: ${usage.projectedOverageUsd.toFixed(2)}
          </span>
        : null}
      </div>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <UsageBar
          label="Context Facts"
          icon="facts"
          consumed={usage.contextFactsConsumed}
          included={usage.contextFactsIncluded}
        />
        <UsageBar
          label="Agent Query Credits"
          icon="credits"
          consumed={usage.agentQueryCreditsConsumed}
          included={usage.agentQueryCreditsIncluded}
        />
      </div>
    </div>
  )
}
