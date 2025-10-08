import { useStore } from '@nanostores/react'
import { Link } from '~/elements/Link'
import { Skeleton } from '~/elements/Skeleton'
import { $currentWorkspacePlan, $paidWorkspace } from '~/features/billing/stores/plans'
import { formatIsoToLocal } from '~/lib/formatters'
import { cn } from '~/lib/utils'

export function CurrentSubscriptionInfo({ className, ...props }: TPolymorphicComponentProps<'div'>) {
  const { currentPlan, loading, validTill, isSubscriptionCancelled } = useStore($currentWorkspacePlan)

  const paidUser = useStore($paidWorkspace)

  return (
    <div className={cn('flex w-full flex-col items-end text-sm', className)} {...props}>
      <Skeleton enabled={loading}>
        <span className="text-semibold">{currentPlan?.name}</span>
      </Skeleton>
      <Skeleton enabled={loading}>
        {paidUser && validTill ?
          <span className="flex gap-1 text-xs">
            {isSubscriptionCancelled ? 'Ends on' : 'Extends on'}
            <Link as="span" size="xsmall">
              {formatIsoToLocal(validTill)}
            </Link>
          </span>
        : <Link as="span" size="xsmall">
            Upgrade
          </Link>
        }
      </Skeleton>
    </div>
  )
}
