import { useState } from 'react'
import { Section, SectionHeader, SectionSubtitle, SectionTitle } from '~/components/Section'

import { ComponentPropsWithoutRef, ReactNode } from 'react'
import cx from 'classnames'
import { Button } from '~/components/Button'
import { ArrowUpRight, Check, Loader } from 'lucide-react'

import Link from 'next/link'
import { links } from '~/config/urls'
import { useBillingData } from '~/hooks/useBillingData'

function Feat({ title, subtitle }: { title: ReactNode; subtitle?: ReactNode }) {
  return (
    <li className="py-3 text-start">
      <div>
        <span className="text-start text-base font-medium">
          <Check className="mr-3 h-4 w-4 shrink-0" />

          {title}
        </span>
      </div>

      {subtitle && <div className="text-content2 pl-7 text-start text-sm">{subtitle}</div>}
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
  featured,
  ...props
}: ComponentPropsWithoutRef<'div'> & {
  title?: ReactNode
  description: ReactNode
  action: ReactNode
  price?: number | 'free'
  featured?: boolean
}) {
  return (
    <article
      className={cx(
        'bg-secondary flex flex-col items-center rounded-xl p-1 text-center shadow-lg first:rounded-l-3xl last:rounded-r-3xl sm:!rounded-xl [&:first-child>div]:rounded-l-[20px] [&:last-child>div]:rounded-r-[20px]',
        featured ? 'from-accent-hover to-accent-orange bg-gradient-to-br' : 'bg-secondary',
        className
      )}
      {...props}
    >
      <div
        className={cx(
          'flex h-full w-full flex-col items-center rounded-lg p-5 sm:!rounded-lg',
          featured ? 'bg-fill' : 'bg-secondary'
        )}
      >
        <h3 className="typography-sm h-5 font-bold">{title}</h3>

        <span className="typography-2xl font-bold uppercase">
          {price !== 'free' && price !== undefined && '$'}
          {price ?? 'Custom'}
        </span>

        <p className="text-content2 mb-3">{description}</p>

        <div className="grid w-full">{action}</div>
        {price !== 'free' && price !== undefined ?
          <p className="text-content3 mt-1 text-sm">No credit card required</p>
        : <p className="mt-1 select-none text-xs text-transparent">-</p>}

        <ul className="mt-5 flex w-full flex-col divide-y">{children}</ul>
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

  const monthlyPricePro = data?.pro?.month?.amount
  const monthlyPriceStart = data?.start?.month?.amount

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

        <SectionSubtitle>
          {variant === Variants.Cloud && 'Start building for free with the power to scale.'}
        </SectionSubtitle>
      </SectionHeader>

      <div className="grid grid-cols-3 gap-3 sm:grid-cols-1 md:grid-cols-2">
        {variant === Variants.Cloud && (
          <>
            <PricingCard
              price="free"
              description="Forever"
              action={
                <Button size="small" variant="secondary" as={Link} href={links.app} target="_blank">
                  Start Building
                  <ArrowUpRight />
                </Button>
              }
            >
              <Feat title="100 000 Records" />
              <Feat title="2 Projects" />
              <Feat title="Community Support" />
            </PricingCard>
            {monthlyPriceStart && (
              <PricingCard
                price={monthlyPriceStart}
                featured
                title="Start"
                description="Monthly"
                action={
                  <Button size="small" variant="accent" as={Link} href={links.app}>
                    Start for Free
                    <ArrowUpRight />
                  </Button>
                }
              >
                <Feat title="400 000 Records" />
                <Feat title="Unlimited Projects" />
                <Feat title="Priority Support" />
              </PricingCard>
            )}
            {monthlyPricePro && (
              <PricingCard
                price={monthlyPricePro}
                featured
                title="Pro"
                description="Monthly"
                action={
                  <Button size="small" variant="accent" as={Link} href={links.app}>
                    Start for Free
                    <ArrowUpRight />
                  </Button>
                }
              >
                <Feat title="1 000 000 Records" />
                <Feat title="Unlimited Projects" />
                <Feat title="Backups" subtitle="Coming soon" />
                <Feat title="Priority Support" />
              </PricingCard>
            )}
          </>
        )}
      </div>
    </Section>
  )
}
