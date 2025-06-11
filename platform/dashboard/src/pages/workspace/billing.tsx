import { useStore } from '@nanostores/react'
import { Check, SparklesIcon } from 'lucide-react'

import type { Plan, PlanId, PlanPeriod } from '~/features/billing/types'

import { Divider } from '~/elements/Divider'
import { Link } from '~/elements/Link'
import { NothingFound } from '~/elements/NothingFound'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Skeleton } from '~/elements/Skeleton'
import { Tab, Tabs, TabsList } from '~/elements/Tabs'
import { CheckoutButton } from '~/features/billing/components/CheckoutButton'
import { FREE_PLAN, PLAN_PERIODS } from '~/features/billing/constants'
import { $availablePlans, $currentPeriod, $currentPlan, $paidUser } from '~/features/billing/stores/plans'
import { isFreePlan } from '~/features/billing/utils'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import { api } from '~/lib/api'
import { currencyFormatters } from '~/lib/formatters'
import { cn, range } from '~/lib/utils'

const benefitsMap: Record<PlanId, Array<{ description?: string; title: string }>> = {
  free: [{ title: '2 Projects' }, { title: '10,000 Records' }, { title: 'Community support' }],
  start: [
    { title: 'Unlimited Projects' },
    { title: 'Bring Your Own Neo4j Database' },
    { title: 'Cypher Query Preview' },
    { title: '100 000 Records', description: 'No limits with your own Neo4j instance' },
    { title: '7-Day Backups', description: 'Coming soon' },
    { title: '3 Team Members', description: 'Additional seats $5/month each' }
  ],
  pro: [
    { title: 'Unlimited Projects' },
    { title: 'Bring Your Own Neo4j Database' },
    { title: 'Cypher Query Preview' },
    { title: '1 000 000 Records', description: 'No limits with your own Neo4j instance' },
    { title: '30-Day Backups', description: 'Coming soon' },
    { title: '10 Team Members', description: 'Additional seats $5/month each' },
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

  const priceAfterDiscount =
    free ? 0
    : currentPeriod === 'annual' ? plan.yearlyPrice / 12
    : plan.monthlyPrice

  const discount = free ? 0 : plan.monthlyPrice * 12 - plan.yearlyPrice

  const hasDiscount = currentPeriod === 'annual' && !free && discount > 0

  return (
    <div
      className={cn(
        'rounded-2xl shadow-lg',
        free ? 'bg-secondary p-1' : 'from-accent-hover to-badge-yellow bg-gradient-to-br p-1',
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
            <span className="from-badge-blue/10 to-badge-blue/30 text-badge-blue flex items-center rounded-full bg-gradient-to-bl px-3 font-mono text-base">
              Save {currencyFormatters.usd.format(discount)}
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
                {hasDiscount && (
                  <span className="text-content3 line-through">
                    {currencyFormatters.usd.format(plan.monthlyPrice * 12)}
                  </span>
                )}{' '}
                {currencyFormatters.usd.format(priceAfterDiscount * 12)}
              </span>
              <span className="text-content2 text-sm">/year</span>
            </div>
          : null}
        </div>

        <div className="flex items-center gap-5">
          <CheckoutButton
            className="w-full min-w-48 font-semibold"
            disabled={active || free}
            id={plan.id}
            variant={free ? 'secondary' : 'accent'}
          >
            {active ?
              'Current Plan'
            : free ?
              'Free'
            : 'Upgrade Plan'}
            {!free && <SparklesIcon />}
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
        {free ?
          <p className={'mb-5'}> </p>
        : <p className={'text-content-secondary'}>
            Free trial period for <strong>14 days</strong> - no payment required
          </p>
        }

        <Divider className="-mx-5 w-[stretch]" />

        <PlanBenefits id={plan.id} />
      </div>
    </div>
  )
}

function SelectPeriod() {
  const currentPeriod = useStore($currentPeriod)

  const plans = useStore($availablePlans)

  const highestDiscount =
    plans ? Math.max(...plans.map((plan) => plan.monthlyPrice * 12 - plan.yearlyPrice)) : 0

  return (
    <Tabs
      className="w-full max-w-xs"
      onValueChange={(period) => $currentPeriod.set(period as PlanPeriod)}
      value={currentPeriod}
    >
      <TabsList className="flex w-full rounded-xl border-none !px-1">
        {PLAN_PERIODS.map((period) => (
          <Tab className="flex-1 justify-center text-center" key={period.value} value={period.value}>
            <span className="capitalize">{period.name}</span>

            {period.value === 'annual' && highestDiscount > 0 && (
              <span className="text-accent text-xs">
                Save {currencyFormatters.usd.format(highestDiscount)}
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
      {!paidUser && <PlanCard active={true} className="order-last sm:order-first" plan={FREE_PLAN} />}

      {plans?.map((plan) => (
        <PlanCard
          active={plan.id === currentPlan?.id}
          className={cn(plan.id === currentPlan?.id ? 'lg:col-span-2' : 'lg:col-span-1')}
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
