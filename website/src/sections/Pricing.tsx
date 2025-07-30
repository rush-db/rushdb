import { useMemo, useState } from 'react'
import { Section, SectionHeader, SectionSubtitle, SectionTitle } from '~/components/Section'

import { ComponentPropsWithoutRef, ReactNode } from 'react'
import cx from 'classnames'
import { Button } from '~/components/Button'
import { ArrowUpRight, Check } from 'lucide-react'

import Link from 'next/link'
import { links } from '~/config/urls'
import { Switch } from '~/components/Switch'
import { PricingComparison } from '~/components/PricingComparison'
import { PricingCalculator } from '~/components/PricingCalculator'
import { BillingData } from '~/components/pricing-types'
import NumberFlow from '@number-flow/react'

interface PricingProps {
  billingData?: BillingData | null
}

function Feat({ title, subtitle }: { title: ReactNode; subtitle?: ReactNode }) {
  return (
    <li className="py-3 text-start">
      <div>
        <span className="text-content2 text-start text-base font-medium">
          <Check className="text-accent mr-3 h-4 w-4 shrink-0" />

          {title}
        </span>
      </div>

      {subtitle && <div className="text-content3 pl-7 text-start text-sm font-medium">{subtitle}</div>}
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
  pricePrefix,
  featured,
  ...props
}: ComponentPropsWithoutRef<'div'> & {
  title?: ReactNode
  description: ReactNode
  action: ReactNode
  headline?: string
  pricePrefix?: string
  price?: number | 'free'
  pricePlaceholder?: string
  featured?: boolean
}) {
  return (
    <article
      className={cx(
        'flex flex-col items-center rounded-xl p-1 text-center first:rounded-l-3xl last:rounded-r-3xl sm:!rounded-xl [&:first-child>div]:rounded-l-[20px] [&:last-child>div]:rounded-r-[20px]',
        featured ? 'border-accent border-2' : 'border-2',
        className
      )}
      {...props}
    >
      <div
        className={cx(
          'flex h-full w-full flex-col items-center justify-between gap-4 rounded-lg p-4 sm:!rounded-lg',
          featured ? 'bg-fill' : ''
        )}
      >
        <div className="w-full p-4">
          <h3 className="typography-md text-accent h-4 font-bold">{title}</h3>
          <div className="mt-2 flex flex-col">
            <p className="typography-sm text-content3 font-bold capitalize">
              {pricePrefix ? pricePrefix : ' '}{' '}
            </p>
            <span className="typography-2xl font-bold capitalize">
              {typeof price === 'number' ?
                <NumberFlow value={price} decimals={0} prefix="$" duration={1} />
              : (pricePlaceholder ?? 'Custom')}
            </span>
          </div>
          <p className="text-content3 text-sm font-medium">{description}</p>
        </div>
        <div className="mb-2 grid w-full">{action}</div>
        <div className="w-full flex-auto text-left">
          <p className="text-content3 mb-2 text-sm font-medium">{headline}</p>

          <ul className="flex w-full flex-col divide-y">{children}</ul>
        </div>
      </div>
    </article>
  )
}

export function Pricing({ billingData }: PricingProps) {
  const [annual, setAnnual] = useState(true)
  const [calculatorBillingType, setCalculatorBillingType] = useState<'onDemand' | 'reserved'>('reserved')
  const [recordsCount, setRecordsCount] = useState(10000)
  const [calculatorTouched, setCalculatorTouched] = useState(false)

  const handleBillingTypeChange = (billingType: 'onDemand' | 'reserved') => {
    setCalculatorBillingType(billingType)
    setAnnual(billingType === 'reserved')
  }

  const handleTierChange = (recordsCount: number) => {
    setCalculatorTouched(true)
    setRecordsCount(recordsCount)
  }

  const getFeaturedCard = () => {
    if (!calculatorTouched) return 'team'
    if (recordsCount <= 10000) return 'start'
    if (recordsCount <= 200000) return 'pro'
    return 'team'
  }

  const featuredCard = getFeaturedCard()

  const proPrice = useMemo(
    () => (annual ? billingData?.pro.annual.amount : billingData?.pro.monthly.amount),
    [annual, billingData]
  )
  const teamPrice = useMemo(
    () => (annual ? billingData!.team[0].reserved.amount : billingData!.team[0].onDemand.amount),
    [annual, billingData]
  )

  return (
    <Section className="container gap-8">
      <SectionHeader className="text-center">
        <SectionTitle className="m-auto max-w-3xl">Simple, Transparent Pricing</SectionTitle>
        <SectionSubtitle className="text-content3">
          Start building for free. Scale when you're ready. No hidden fees.
        </SectionSubtitle>
      </SectionHeader>
      <div className="mx-auto mb-4 flex items-center justify-center gap-4">
        <p className="text-content2 typography-base font-medium">Monthly</p>
        <Switch
          checked={calculatorBillingType === 'reserved'}
          onCheckedChange={(checked) => handleBillingTypeChange(checked ? 'reserved' : 'onDemand')}
        />
        <p className="text-content2 typography-base font-medium">
          Annual
          <span className="text-accent-green bg-accent-green/10 ml-2 rounded-full px-2 py-0.5 text-sm font-bold">
            Save up to 36%
          </span>
        </p>
      </div>
      <div className="mb-8 grid grid-cols-4 gap-4 sm:grid-cols-1 md:grid-cols-2 lg:grid-cols-4">
        <PricingCard
          price="free"
          pricePrefix="forever"
          title="Start"
          featured={featuredCard === 'start'}
          description="Perfect for side projects and learning"
          headline="Everything you need to start:"
          action={
            <Button size="small" variant="outline" as={Link} href={links.app} target="_blank">
              Start Building Free
              <ArrowUpRight />
            </Button>
          }
        >
          <Feat title="Up to 10,000 records" subtitle="Perfect for prototypes and small apps" />
          <Feat title="2 projects" subtitle="No time limits, no feature restrictions" />
          <Feat title="Full REST API and SDKs" subtitle="Complete access to all query capabilities" />
          <Feat title="Smart CDN" subtitle="Global API endpoints for faster access" />
          <Feat title="Community support" subtitle="Get help from our developer community" />
        </PricingCard>

        <PricingCard
          price={proPrice}
          pricePrefix="from"
          title="Pro"
          featured={featuredCard === 'pro'}
          description="Perfect for growing projects"
          headline="Everything in Start, plus:"
          action={
            <Button
              size="small"
              variant={featuredCard === 'pro' ? 'accent' : 'outline'}
              as={Link}
              href={links.app}
            >
              Upgrade Now
              <ArrowUpRight />
            </Button>
          }
        >
          <Feat
            title="Up to 200,000 records"
            subtitle="On shared infrastructure. Unlimited with your own Neo4j instance"
          />
          <Feat title="Unlimited projects" subtitle="Record limit applies collectively across all projects" />
          <Feat title="Bring your Neo4j" subtitle="Connect Neo4j Aura or own instance" />
          <Feat title="3 team members" subtitle="then $10 per member" />
        </PricingCard>

        <PricingCard
          price={teamPrice}
          pricePrefix="from"
          title="Team"
          featured={featuredCard === 'team'}
          description="For teams and advanced features"
          headline="Everything in Pro, plus:"
          action={
            <Button
              size="small"
              variant={featuredCard === 'team' ? 'accent' : 'outline'}
              as={Link}
              href="mailto:hello@rushdb.com"
            >
              Upgrade Now
              <ArrowUpRight />
            </Button>
          }
        >
          <Feat title="Up to 1B+ records" subtitle="Instant scale up with a single click—grow as needed" />
          <Feat title="Dedicated instances" subtitle="Guaranteed performance, security, and throughput" />
          <Feat title="10 team members" subtitle="then $25 per member" />
          <Feat title="Daily backups stored for 14 days" subtitle="Coming soon" />
          <Feat title="SSO" subtitle="Enterprise-grade authentication (coming soon)" />
          <Feat title="Priority support" subtitle="Fast-track access to expert help" />
        </PricingCard>
        <PricingCard
          pricePlaceholder="Contact Us"
          title="Enterprise"
          pricePrefix="&nbsp;"
          description="For large-scale deployments"
          headline="Everything in Team, plus:"
          action={
            <Button size="small" variant="outline" as={Link} href="mailto:hi@rushdb.com">
              Contact Us
              <ArrowUpRight />
            </Button>
          }
        >
          <Feat title="Unlimited everything" subtitle="No limits on records, projects, or usage" />
          <Feat title="Dedicated support" subtitle="Phone and chat support" />
          <Feat title="SLA guarantees" subtitle="Uptime and performance guarantees" />
          <Feat title="Custom deployment" subtitle="On-premise or private cloud options" />
        </PricingCard>
      </div>
      <PricingCalculator
        tieredPricingData={billingData!.team}
        onTierChange={handleTierChange}
        billingType={'reserved'}
      />
      <PricingComparison className="mt-16" />
    </Section>
  )
}
