import { isFreePlan } from '~/features/billing/utils.ts'
import { useStore } from '@nanostores/react'
import { $currentPeriod } from '~/features/billing/stores/plans.ts'
import { cn } from '~/lib/utils.ts'
import { currencyFormatters } from '~/lib/formatters.ts'
import { SparklesIcon } from 'lucide-react'

import { Divider } from '~/elements/Divider.tsx'
import { PlanBenefits } from '~/components/billing/PlanBenefits.tsx'
import { CheckoutButton } from '~/components/billing/CheckoutButton.tsx'
import { Message } from '~/elements/Message.tsx'

export function PlanCard({
  plan,
  active,
  className,
  perProject,
  ...props
}: TPolymorphicComponentProps<'div', { active: boolean; plan: any; perProject?: boolean }>) {
  const free = isFreePlan(plan)

  const currentPeriod = useStore($currentPeriod)

  const priceAfterDiscount =
    free ? 0
    : currentPeriod === 'annual' ? plan.yearlyPrice
    : plan.monthlyPrice

  const discount =
    currentPeriod === 'annual' ?
      Math.round(((plan.monthlyPrice - plan.yearlyPrice) / plan.monthlyPrice) * 100)
    : 0

  const hasDiscount = currentPeriod === 'annual' && !free && discount > 0

  return (
    <div
      className={cn(
        'rounded-2xl border shadow-lg',
        free || plan.id === 'team' ? 'border-secondary' : 'border-accent',
        'flex h-full flex-col items-start gap-5 p-5',
        free ? 'bg-secondary' : 'bg-fill',
        // free ? 'bg-secondary p-0.5' : 'from-accent-hover to-badge-yellow bg-gradient-to-br p-0.5',
        className
      )}
      {...props}
    >
      <div className="flex w-full justify-between gap-3">
        <h2 className="text-bold text-lg font-bold">{plan.name}</h2>
        {hasDiscount && (
          <span className="from-badge-blue/10 to-badge-blue/30 text-badge-blue flex items-center rounded-full bg-gradient-to-bl px-3 font-mono text-base">
            Save {discount}%
          </span>
        )}
      </div>
      <div className="flex w-full justify-between">
        {(currentPeriod === 'month' || free) && (
          <div className="block">
            <span className="font-mono text-3xl font-bold">
              {currencyFormatters.usd.format(priceAfterDiscount)}
            </span>
            <span className="text-content2 text-sm">/{free ? 'forever' : 'month'}</span>
          </div>
        )}
        {hasDiscount && !free ?
          <div className="block">
            <span className="font-mono text-3xl font-bold">
              {currencyFormatters.usd.format(priceAfterDiscount)}
              {hasDiscount && (
                <span className="text-content3 text-lg line-through">
                  {currencyFormatters.usd.format(plan.monthlyPrice)}
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
        : <CheckoutButton
            className="w-full min-w-48 justify-center font-semibold"
            disabled={active || free}
            priceId={currentPeriod === 'month' ? plan.monthlyPriceId : plan.yearlyPriceId}
            variant={free ? 'secondary' : 'accent'}
          >
            {active ?
              'Current Plan'
            : free ?
              'Free'
            : 'Upgrade Plan'}
            {!free && <SparklesIcon />}
          </CheckoutButton>
        }
      </div>

      <Divider className="-mx-5 w-[stretch]" />

      <PlanBenefits id={plan.id} />
    </div>
  )
}
