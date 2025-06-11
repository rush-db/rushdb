import { useMemo, useState } from 'react'
import { Section, SectionHeader, SectionSubtitle, SectionTitle } from '~/components/Section'

import { ComponentPropsWithoutRef, ReactNode } from 'react'
import cx from 'classnames'
import { Button } from '~/components/Button'
import { ArrowUpRight, Check, Loader } from 'lucide-react'

import Link from 'next/link'
import { links } from '~/config/urls'
import { useBillingData } from '~/hooks/useBillingData'
import { Switch } from '~/components/Switch'

function Feat({ title, subtitle }: { title: ReactNode; subtitle?: ReactNode }) {
  return (
    <li className="py-3 text-start">
      <div>
        <span className="text-content2 text-start text-base font-medium">
          <Check className="text-accent mr-3 h-4 w-4 shrink-0" />

          {title}
        </span>
      </div>

      {subtitle && <div className="text-content3 pl-7 text-start text-sm">{subtitle}</div>}
    </li>
  )
}

function PricingCard({
  title,
  description,
  action,
  className,
  price,
  children,
  headline,
  pricePlaceholder,
  featured,
  ...props
}: ComponentPropsWithoutRef<'div'> & {
  title?: ReactNode
  description: ReactNode
  action: ReactNode
  headline?: string
  price?: number | 'free'
  pricePlaceholder?: string
  featured?: boolean
}) {
  return (
    <article
      className={cx(
        'bg-secondary flex flex-col items-center rounded-xl p-1 text-center first:rounded-l-3xl last:rounded-r-3xl sm:!rounded-xl [&:first-child>div]:rounded-l-[20px] [&:last-child>div]:rounded-r-[20px]',
        featured ? 'border-accent border-2' : 'bg-secondary border-2',
        className
      )}
      {...props}
    >
      <div
        className={cx(
          'flex h-full w-full flex-col items-center justify-between gap-4 rounded-lg p-4 sm:!rounded-lg',
          featured ? 'bg-fill' : 'bg-secondary'
        )}
      >
        <div className="w-full p-4">
          <h3 className="typography-sm text-accent h-5 font-bold">{title}</h3>

          <span className="typography-2xl font-bold capitalize">
            {price !== 'free' && price !== undefined && '$'}
            {price ?? pricePlaceholder ?? 'Custom'}
          </span>

          <p className="text-content3">{description}</p>
        </div>

        <div className="w-full flex-auto text-left">
          <p className="text-content3 mb-2">&nbsp;{headline}</p>

          <ul className="flex w-full flex-col divide-y">{children}</ul>
        </div>
        <div className="grid w-full">{action}</div>
      </div>
    </article>
  )
}

enum Variants {
  Cloud = 'cloud',
  OnPremise = 'on-premise'
}

export function Pricing() {
  const [variant] = useState<Variants>(Variants.Cloud)
  const { loading, data } = useBillingData()

  const [annual, setAnnual] = useState(false)

  const prices = useMemo(
    () => ({
      start: {
        month: data?.start?.month?.amount,
        annual: data?.start?.annual?.amount
      },
      pro: {
        month: data?.pro?.month?.amount,
        annual: data?.pro?.annual?.amount
      }
    }),
    [data]
  )

  const startPrice = useMemo(() => prices.start[annual ? 'annual' : 'month'], [annual, prices])
  const proPrice = useMemo(() => prices.pro[annual ? 'annual' : 'month'], [annual, prices])

  if (loading) {
    return (
      <div className="container grid h-full min-h-dvh w-full place-content-center items-center">
        <Loader />
      </div>
    )
  }

  return (
    <Section className="container">
      <SectionHeader className="text-center">
        <SectionTitle className="m-auto max-w-3xl">Pricing</SectionTitle>
        <SectionSubtitle className="text-content3">
          Start building for free with the power to scale
        </SectionSubtitle>
      </SectionHeader>

      <div className="mx-auto flex items-center gap-4">
        <p className="text-content3">Monthly</p>
        <Switch checked={annual} onCheckedChange={setAnnual} aria-readonly />
        <p className="text-content3">Annually</p>
      </div>
      <div className="grid grid-cols-4 gap-4 sm:grid-cols-1 md:grid-cols-2">
        <PricingCard
          price="free"
          title="Free"
          description="Forever"
          headline={' '}
          action={
            <Button size="small" variant="outline" as={Link} href={links.app} target="_blank">
              Continue Free
              <ArrowUpRight />
            </Button>
          }
        >
          <Feat title="Unlimited API Requests" />
          <Feat title="10 000 Records" />
          <Feat title="2 Projects" subtitle="Projects are never paused, available for commercial use" />
          <Feat title="1 Team Member" />
          <Feat title="Community Support" />
        </PricingCard>
        {startPrice && (
          <PricingCard
            price={startPrice}
            title="Start"
            description={annual ? 'Annually' : 'Monthly'}
            headline="Everything in Free, plus:"
            action={
              <Button size="small" variant="outline" as={Link} href={links.app}>
                Start Building
                <ArrowUpRight />
              </Button>
            }
          >
            <Feat title="Unlimited Projects" />
            <Feat title="Bring Your Own Neo4j Database" />
            <Feat title="Cypher Query Preview" />
            <Feat title="100 000 Records" subtitle="No limits with your own Neo4j instance" />
            <Feat title="7-Day Backup" subtitle="Coming soon" />
            <Feat title="3 Team Members" subtitle="Additional seats $5/month each" />
          </PricingCard>
        )}
        {proPrice && (
          <PricingCard
            price={proPrice}
            title="Pro"
            featured
            description={annual ? 'Annually' : 'Monthly'}
            headline="Everything in Start, plus:"
            action={
              <Button size="small" variant="accent" as={Link} href={links.app}>
                Start Building
                <ArrowUpRight />
              </Button>
            }
          >
            <Feat title="Unlimited Projects" />
            <Feat title="Bring Your Own Neo4j Database" />
            <Feat title="Cypher Query Preview" />
            <Feat title="1 000 000 Records" subtitle="No limits with your own Neo4j instance" />
            <Feat title="30-Day Backup" subtitle="Coming soon" />
            <Feat title="10 Team Members" subtitle="Additional seats $5/month each" />
            <Feat title="Priority Support" />
          </PricingCard>
        )}
        <PricingCard
          className="opacity-50"
          title="Enterprise"
          pricePlaceholder={'Custom'}
          description="Coming soon"
          headline="Everything in Pro, plus:"
          action={null}
        >
          <Feat title="Unlimited Everything" />
          <Feat title="Premium Support" />
          <Feat title="Uptime SLAs" />
          <Feat title="Customizations" />
        </PricingCard>
      </div>
    </Section>
  )
}
