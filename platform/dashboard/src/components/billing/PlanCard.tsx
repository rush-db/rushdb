import { isFreePlan } from '~/features/billing/utils.ts'
import { useStore } from '@nanostores/react'
import { $currentPeriod } from '~/features/billing/stores/plans.ts'
import { cn } from '~/lib/utils.ts'
import { currencyFormatters } from '~/lib/formatters.ts'
import { SparklesIcon } from 'lucide-react'

import { Divider } from '~/elements/Divider.tsx'
import { Button } from '~/elements/Button.tsx'
import { Skeleton } from '~/elements/Skeleton.tsx'
import { PlanBenefits } from '~/components/billing/PlanBenefits.tsx'
import { CheckoutButton } from '~/components/billing/CheckoutButton.tsx'
import { Message } from '~/elements/Message.tsx'
import type { DisplayPlan } from '~/features/billing/types.ts'

export type PlanRelation = 'current' | 'upgrade' | 'downgrade'

// Placeholder card shown while pricing data loads, mirroring PlanCard's layout
// (title, price, CTA, divider, benefit rows) so the grid doesn't reflow.
export function PlanCardSkeleton({ className }: { className?: string }) {
  return (
    <div
      className={cn(
        'border-secondary bg-fill relative flex h-full flex-col items-start gap-5 rounded-2xl border p-5 shadow-lg',
        className
      )}
    >
      <Skeleton enabled className="h-6 w-24 rounded-md" />
      <Skeleton enabled className="h-9 w-28 rounded-md" />
      <Skeleton enabled className="h-10 w-full rounded-md" />
      <Divider className="-mx-5 w-[stretch]" />
      <div className="flex w-full flex-col gap-3">
        {Array.from({ length: 5 }).map((_, index) => (
          <Skeleton enabled className="h-4 w-3/4 rounded" key={index} />
        ))}
      </div>
    </div>
  )
}

export function PlanCard({
  plan,
  relation,
  recommended,
  className,
  perProject,
  onManage,
  onAction,
  ...props
}: TPolymorphicComponentProps<
  'div',
  {
    relation: PlanRelation
    recommended?: boolean
    plan: DisplayPlan
    perProject?: boolean
    onManage?: () => void
    onAction?: () => void
  }
>) {
  const free = isFreePlan(plan)
  const inquiryOnly = Boolean(plan.inquiryOnly)
  const isCurrent = relation === 'current'

  const currentPeriod = useStore($currentPeriod)

  const priceAfterDiscount =
    free ? 0
    : inquiryOnly ? undefined
    : currentPeriod === 'annual' ? plan.yearlyPrice
    : plan.monthlyPrice

  const discount =
    currentPeriod === 'annual' && !inquiryOnly && plan.monthlyPrice && plan.yearlyPrice ?
      Math.round(((plan.monthlyPrice - plan.yearlyPrice) / plan.monthlyPrice) * 100)
    : 0

  const hasDiscount = currentPeriod === 'annual' && !free && !inquiryOnly && discount > 0
  const displayPrice = priceAfterDiscount ?? 0
  const monthlyPrice = plan.monthlyPrice ?? 0
  const priceId = currentPeriod === 'month' ? plan.monthlyPriceId : plan.yearlyPriceId

  return (
    <div
      className={cn(
        'relative flex h-full flex-col items-start gap-5 rounded-2xl border p-5 shadow-lg',
        recommended ? 'border-accent ring-accent/30 ring-1'
        : isCurrent ? 'border-content/40'
        : 'border-secondary',
        free ? 'bg-secondary' : 'bg-fill',
        className
      )}
      {...props}
    >
      <div className="flex w-full items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <h2 className="text-bold text-lg font-bold">{plan.name}</h2>
          {recommended ?
            <span className="bg-accent text-accent-contrast rounded-full px-2.5 py-0.5 text-xs font-semibold">
              Recommended
            </span>
          : isCurrent ?
            <span className="text-content2 rounded-full border px-2.5 py-0.5 text-xs font-semibold">
              Current
            </span>
          : null}
        </div>
        {hasDiscount && (
          <span className="from-badge-blue/10 to-badge-blue/30 text-badge-blue flex items-center rounded-full bg-gradient-to-bl px-3 font-mono text-base">
            Save {discount}%
          </span>
        )}
      </div>

      <div className="flex w-full justify-between">
        {inquiryOnly ?
          <div className="block">
            <span className="font-mono text-3xl font-bold">Custom</span>
          </div>
        : (currentPeriod === 'month' || free) && (
            <div className="block">
              <span className="font-mono text-3xl font-bold">
                {currencyFormatters.usd.format(displayPrice)}
              </span>
              <span className="text-content2 text-sm">/{free ? 'forever' : 'month'}</span>
            </div>
          )
        }
        {hasDiscount && !free ?
          <div className="block">
            <span className="font-mono text-3xl font-bold">
              {currencyFormatters.usd.format(displayPrice)}
              {hasDiscount && (
                <span className="text-content3 text-lg line-through">
                  {currencyFormatters.usd.format(monthlyPrice)}
                </span>
              )}
            </span>
            <span className="text-content2 text-sm">/month</span>
          </div>
        : null}
      </div>

      <div className="flex w-full items-center gap-5">
        {perProject ?
          <Message as="div" variant="info" size="small">
            This plan bills on a per-project basis. To use it, create a new project and select Managed Setup
            in the project creation form.
          </Message>
        : inquiryOnly ?
          <Button
            className="w-full min-w-48 justify-center font-semibold"
            onClick={onAction}
            variant="accent"
          >
            {plan.ctaLabel}
            <SparklesIcon />
          </Button>
        : isCurrent ?
          <Button className="w-full min-w-48 justify-center font-semibold" disabled variant="secondary">
            Current plan
          </Button>
        : relation === 'downgrade' ?
          <Button
            className="w-full min-w-48 justify-center font-semibold"
            onClick={onManage}
            variant="secondary"
          >
            {free ? 'Switch to Free' : `Switch to ${plan.name}`}
          </Button>
        : <CheckoutButton
            billingPeriod={currentPeriod === 'annual' ? 'annual' : 'monthly'}
            className="w-full min-w-48 justify-center font-semibold"
            planName={plan.name}
            priceId={priceId ?? ''}
            variant="accent"
          >
            Upgrade to {plan.name}
            <SparklesIcon />
          </CheckoutButton>
        }
      </div>

      <Divider className="-mx-5 w-[stretch]" />

      <PlanBenefits benefits={plan.benefits} id={plan.id} />
    </div>
  )
}
