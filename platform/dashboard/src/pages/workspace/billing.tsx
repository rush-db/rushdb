import { useStore } from '@nanostores/react'
import { Check } from 'lucide-react'

import type { Plan, PlanId, PlanPeriod } from '~/features/billing/types'

import { Divider } from '~/elements/Divider'
import { Link } from '~/elements/Link'
import { NothingFound } from '~/elements/NothingFound'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Skeleton } from '~/elements/Skeleton'
import { Tab, Tabs, TabsList } from '~/elements/Tabs'
import { CheckoutButton } from '~/features/billing/components/CheckoutButton'
import { FREE_PLAN, PLAN_PERIODS } from '~/features/billing/constants'
import {
  $availablePlans,
  $currentPeriod,
  $currentPlan,
  $paidUser
} from '~/features/billing/stores/plans'
import { isFreePlan } from '~/features/billing/utils'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import { api } from '~/lib/api'
import { currencyFormatters, percentFormatter } from '~/lib/formatters'
import { cn, range } from '~/lib/utils'

const benefitsMap: Record<
  PlanId,
  Array<{ description?: string; title: string }>
> = {
  free: [
    { title: '2 Projects' },
    { title: '10.000 Records' },
    { title: 'Unlimited API Requests', description: 'Up to 10 RPS' },
    { title: 'Community support' }
  ],
  pro: [
    { title: 'Unlimited Projects' },
    { title: '1.000.000 Records', description: 'then $1 per 10.000 Records' },
    { title: 'Unlimited API Requests', description: 'No RPS Limits' },
    { title: 'Priority Support' }
  ]
}

function PlanBenefits({ id }: { id: PlanId }) {
  const benefits = benefitsMap[id]

  return (
    <ul className="flex flex-col gap-1">
      {benefits.map((benefit) => (
        <li className="flex flex-col" key={benefit.title}>
          <div className="mb-2 flex items-center gap-2">
            <Check className="h-4 w-4" />{' '}
            <div>
              <p>{benefit.title}</p>
              <p className="text-content3">{benefit.description}</p>
            </div>
          </div>
        </li>
      ))}
    </ul>
  )
}

function PlanCard({
  plan,
  active,
  className,
  ...props
}: TPolymorphicComponentProps<'div', { active: boolean; plan: Plan }>) {
  const free = isFreePlan(plan)

  const paidUser = useStore($paidUser)
  const currentPeriod = useStore($currentPeriod)

  const priceAfterDiscount = free
    ? 0
    : (currentPeriod === 'annual' ? 1 - plan.annualDiscount : 1) * plan.price

  const discount = free ? 0 : plan.price - priceAfterDiscount

  const hasDiscount = currentPeriod === 'annual' && !free && discount > 0

  return (
    <div
      className={cn(
        'rounded-2xl shadow-lg',
        free
          ? 'bg-secondary p-1'
          : 'bg-gradient-to-br from-accent-hover to-badge-yellow p-1',
        className
      )}
      {...props}
    >
      <div
        className={cn(
          'flex h-full flex-col items-start gap-5 rounded-[calc(var(--radius)+6px)] p-5',
          free ? 'bg-secondary' : 'bg-fill'
        )}
      >
        <div className="flex w-full justify-between gap-3">
          <h2 className="text-bold text-lg font-bold">{plan.name}</h2>
          {hasDiscount && (
            <span className="flex items-center rounded-full bg-gradient-to-bl  from-badge-blue/10 to-badge-blue/30 px-3 font-mono text-base text-badge-blue">
              Save {currencyFormatters.usd.format(discount * 12)} with annual
              discount
            </span>
          )}
        </div>

        <div className="flex w-full justify-between">
          <div className="block">
            <span className="font-mono text-3xl font-bold">
              {hasDiscount && (
                <span className="text-content3 line-through">
                  {currencyFormatters.usd.format(plan.price)}
                </span>
              )}{' '}
              {currencyFormatters.usd.format(priceAfterDiscount)}
            </span>
            <span className="text-sm text-content2">
              /{free ? 'forever' : 'month'}
            </span>
          </div>
          {hasDiscount && !free ? (
            <div className="block">
              <span className="font-mono text-3xl font-bold">
                {hasDiscount && (
                  <span className="text-content3 line-through">
                    {currencyFormatters.usd.format(plan.price * 12)}
                  </span>
                )}{' '}
                {currencyFormatters.usd.format(priceAfterDiscount * 12)}
              </span>
              <span className="text-sm text-content2">/year</span>
            </div>
          ) : null}
        </div>

        <div className="flex items-center gap-5">
          <CheckoutButton
            className="min-w-48"
            disabled={active || free}
            id={plan.id}
            variant={free ? 'secondary' : 'accent'}
          >
            {active ? 'Current Plan' : free ? 'Free' : 'Upgrade'}
          </CheckoutButton>
          {paidUser && active && (
            <form
              onSubmit={async (event) => {
                event.preventDefault()
                const { redirectUrl } = await api.billing.createPortalSession({
                  returnUrl: window.location.href
                })
                if (redirectUrl) {
                  window.location.replace(redirectUrl)
                }
              }}
            >
              <Link as="button" type="submit">
                Manage Subscription
              </Link>
            </form>
          )}
        </div>

        <Divider className="-mx-5 w-[stretch]" />

        <PlanBenefits id={plan.id} />
      </div>
    </div>
  )
}

function SelectPeriod() {
  const currentPeriod = useStore($currentPeriod)

  const plans = useStore($availablePlans)

  const highestDiscount = plans
    ? Math.max(...plans.map((plan) => plan.annualDiscount))
    : 0

  return (
    <Tabs
      className="w-full max-w-xs"
      onValueChange={(period) => $currentPeriod.set(period as PlanPeriod)}
      value={currentPeriod}
    >
      <TabsList className="flex w-full rounded-xl border-none !px-1">
        {PLAN_PERIODS.map((period) => (
          <Tab
            className="flex-1 justify-center text-center"
            key={period.value}
            value={period.value}
          >
            <span className="capitalize">{period.name}</span>

            {period.value === 'annual' && highestDiscount > 0 && (
              <span className="text-xs text-accent">
                Up to -{percentFormatter.format(highestDiscount)}
              </span>
            )}
          </Tab>
        ))}
      </TabsList>
    </Tabs>
  )
}

function Plans() {
  const plans = useStore($availablePlans)
  const { currentPlan } = useStore($currentPlan)
  const paidUser = useStore($paidUser)

  return (
    <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3">
      <PlanCard
        active={!paidUser}
        className="order-last sm:order-first"
        plan={FREE_PLAN}
      />

      {plans?.map((plan) => (
        <PlanCard
          active={plan.id === currentPlan?.id}
          className="lg:col-span-2"
          key={plan.id}
          plan={plan}
        />
      ))}

      {!plans &&
        range(1).map((index) => (
          <Skeleton className="lg:col-span-2" enabled key={index}>
            <PlanCard active={false} plan={FREE_PLAN} />
          </Skeleton>
        ))}

      {!plans && <NothingFound />}
    </div>
  )
}

export function WorkspaceBillingPage() {
  return (
    <WorkspacesLayout>
      <PageHeader contained>
        <PageTitle>Upgrade Plan</PageTitle>
        <SelectPeriod />
      </PageHeader>

      <PageContent className="gap-5" contained>
        <Plans />
      </PageContent>
    </WorkspacesLayout>
  )
}
