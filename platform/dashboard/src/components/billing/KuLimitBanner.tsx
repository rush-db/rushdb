import { useStore } from '@nanostores/react'
import { AlertTriangle, Zap } from 'lucide-react'

import { $user } from '~/features/auth/stores/user'
import { useWorkspaceUsageQuery } from '~/features/billing/hooks/useBillingHooks'
import { Button } from '~/elements/Button'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'

function formatKu(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n % 1_000_000 === 0 ? 0 : 1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(n % 1_000 === 0 ? 0 : 1)}K`
  return n.toLocaleString()
}

/**
 * Full-width banner shown inside WorkspacesLayout when a cloud workspace is
 * approaching or has exceeded its KU limit.
 *
 * - Hidden on self-hosted instances ($workspaceUsage returns undefined)
 * - Hidden for usage/overage billing models (no hard cap)
 * - Only visible to workspace owners
 */
export function KuLimitBanner() {
  const currentUser = useStore($user)
  const { data: usage } = useWorkspaceUsageQuery()

  const isOwner = currentUser.currentScope?.role === 'owner'

  // $workspaceUsage already returns undefined for self-hosted — no extra check needed
  if (!isOwner || !usage) return null

  // Only for plans with hard limits (free / fixed billing model)
  if (usage.billingModel !== 'fixed' || usage.kuIncluded === null) return null

  const cap = usage.kuIncluded
  const pct = (usage.kuConsumed / cap) * 100
  const isExceeded = usage.kuConsumed >= cap
  const isApproaching = !isExceeded && pct >= 90

  if (!isExceeded && !isApproaching) return null

  return (
    <div
      role="alert"
      className={cn(
        'flex items-center gap-3 px-4 py-2.5 text-sm font-medium',
        isExceeded ?
          'border-b border-danger/20 bg-danger/15 text-danger'
        : 'border-b border-warning/20 bg-warning/10 text-warning'
      )}
    >
      {isExceeded ?
        <AlertTriangle className="h-4 w-4 shrink-0" />
      : <Zap className="h-4 w-4 shrink-0" />}

      <span className="flex-1">
        {isExceeded ?
          <>
            <strong>KU limit reached.</strong> You've used{' '}
            <span className="font-mono">
              {formatKu(usage.kuConsumed)}&thinsp;/&thinsp;{formatKu(cap)} KU
            </span>
            — new writes are blocked until your billing period resets. Upgrade to continue.
          </>
        : <>
            <strong>Approaching KU limit.</strong> You've used{' '}
            <span className="font-mono">
              {formatKu(usage.kuConsumed)}&thinsp;/&thinsp;{formatKu(cap)} KU
            </span>{' '}
            ({pct.toFixed(0)}%). Consider upgrading before you hit the cap.
          </>
        }
      </span>

      <Button
        as="a"
        href={getRoutePath('workspaceBilling')}
        size="small"
        variant={isExceeded ? 'primary' : 'secondary'}
        className="shrink-0"
      >
        Upgrade Plan
      </Button>
    </div>
  )
}
