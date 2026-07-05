import { Link } from '~/elements/Link'
import { Skeleton } from '~/elements/Skeleton'
import { useCurrentWorkspacePlan } from '~/features/billing/hooks/useBillingHooks'
import { useCurrentWorkspaceQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { isFreePlan } from '~/features/billing/utils'
import { formatIsoToLocal } from '~/lib/formatters'
import { cn } from '~/lib/utils'

export function CurrentSubscriptionInfo({ className, ...props }: TPolymorphicComponentProps<'div'>) {
  const { currentPlan, loading } = useCurrentWorkspacePlan()
  const { data: workspace } = useCurrentWorkspaceQuery()
  const validTill = workspace?.validTill
  const isSubscriptionCancelled = workspace?.isSubscriptionCancelled
  const paidUser = currentPlan && !isFreePlan(currentPlan)

  return (
    <div className={cn('flex w-full flex-col items-end text-sm', className)} {...props}>
      <Skeleton enabled={loading}>
        <span className="text-semibold">{currentPlan?.name}</span>
      </Skeleton>
      <Skeleton enabled={loading}>
        {paidUser && isSubscriptionCancelled && validTill ?
          <span className="flex gap-1 text-xs">
            Ends on
            <Link as="span" size="xsmall">
              {formatIsoToLocal(validTill)}
            </Link>
          </span>
        : paidUser ?
          <span className="text-xs text-content3">Active subscription</span>
        : <Link as="span" size="xsmall">
            Upgrade
          </Link>
        }
      </Skeleton>
    </div>
  )
}
