import { useMemo, useState } from 'react'
import { Section, SectionHeader, SectionSubtitle, SectionTitle } from '~/components/Section'

import { ComponentPropsWithoutRef, ReactNode } from 'react'
import cx from 'classnames'
import { Button } from '~/components/Button'
import { ArrowUpRight, Check, ChevronDown, Database, GitBranch, Search, Zap } from 'lucide-react'

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

function KuDefinition() {
  return (
    <div className="border-accent/30 bg-accent/5 w-full rounded-2xl border p-6 md:p-8">
      <div className="mb-6 flex flex-wrap items-start justify-between gap-4">
        <div>
          <div className="mb-1 flex items-center gap-2">
            <Zap className="text-accent h-5 w-5" />
            <h4 className="text-content text-lg font-bold">What is a Knowledge Unit (KU)?</h4>
          </div>
          <p className="text-content3 max-w-2xl text-sm font-medium">
            A <strong className="text-content2">Knowledge Unit</strong> is RushDB's atomic measure of
            structured knowledge stored during a write operation. KU accumulates from records created,
            properties stored, and relationships formed between records. Standard reads and queries are always
            free. Compute-intensive operations (vector search, raw Cypher, multi-hop traversals) consume a
            small amount of KU.
          </p>
        </div>
        <span className="text-accent border-accent/40 shrink-0 rounded-full border px-3 py-1 text-xs font-bold uppercase tracking-wide">
          KU = records · properties · links
        </span>
      </div>
      <div className="grid grid-cols-3 gap-4 sm:grid-cols-1 md:grid-cols-3">
        <div className="bg-fill2 rounded-xl p-4">
          <div className="mb-2 flex items-center gap-2">
            <Database className="text-accent h-4 w-4 shrink-0" />
            <span className="text-content text-sm font-semibold">Flat records</span>
          </div>
          <p className="text-content3 text-xs leading-relaxed">
            A record with <strong className="text-content2">10 properties</strong> costs{' '}
            <strong className="text-accent">10 KU</strong> per write. Simple and predictable.
          </p>
        </div>
        <div className="bg-fill2 rounded-xl p-4">
          <div className="mb-2 flex items-center gap-2">
            <GitBranch className="text-accent h-4 w-4 shrink-0" />
            <span className="text-content text-sm font-semibold">Nested objects</span>
          </div>
          <p className="text-content3 text-xs leading-relaxed">
            Nested objects are{' '}
            <strong className="text-content2">decomposed into separate linked records</strong>. Each child
            record contributes its own property KU, and each link between records adds a small relationship
            cost.
          </p>
        </div>
        <div className="bg-fill2 rounded-xl p-4">
          <div className="mb-2 flex items-center gap-2">
            <Search className="text-accent h-4 w-4 shrink-0" />
            <span className="text-content text-sm font-semibold">Standard reads free</span>
          </div>
          <p className="text-content3 text-xs leading-relaxed">
            Standard queries and reads <strong className="text-content2">never consume KU</strong>.
            Compute-intensive operations (vector search, raw Cypher, multi-hop traversals) consume a small
            amount of KU. A small daily storage footprint applies to data at rest.
          </p>
        </div>
      </div>
    </div>
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

function FaqItem({ question, answer }: { question: string; answer: ReactNode }) {
  const [isOpen, setIsOpen] = useState(false)

  return (
    <div className="border-stroke border-b last:border-b-0">
      <button
        className="text-content hover:text-accent flex w-full items-center justify-between py-5 text-left font-medium transition-colors"
        onClick={() => setIsOpen(!isOpen)}
      >
        <span className="text-base font-semibold">{question}</span>
        <ChevronDown className={cx('text-accent h-5 w-5 transition-transform', isOpen && 'rotate-180')} />
      </button>
      {isOpen && <div className="text-content3 pb-5 text-sm leading-relaxed">{answer}</div>}
    </div>
  )
}

function PricingFaq() {
  return (
    <div className="bg-fill2 mx-auto w-full max-w-4xl rounded-2xl border p-8">
      <div className="mb-8 text-center">
        <h3 className="text-content mb-2 text-2xl font-bold">Frequently Asked Questions</h3>
        <p className="text-content3 text-sm font-medium">Everything you need to know about RushDB pricing</p>
      </div>
      <div className="divide-y">
        <FaqItem
          question="What happens when I hit my KU limit on the Free plan?"
          answer={
            <>
              On the Free plan (100K KU/month), once you reach your limit, writes pause until your next
              billing period — reads and standard queries always continue. We'll send you email notifications
              at 75%, 90%, and 100% usage to keep you informed.
            </>
          }
        />
        <FaqItem
          question="How does overage billing work on the Pro plan?"
          answer={
            <>
              Pro includes 10 million KU per month for ${' '}
              <span className="text-content font-semibold">$29/month</span>. If you exceed this allowance,
              you&apos;ll be charged{' '}
              <span className="text-content font-semibold">$3 per additional million KU</span>. There's no
              hard limit — your applications keep running, and we bill you for actual usage at the end of the
              month. For example, if you use 12M KU in a month: $29 base + (2M × $3) = $35 total.
            </>
          }
        />
        <FaqItem
          question="What's the difference between Pro and Scale plans?"
          answer={
            <>
              <div className="mb-3">
                <strong className="text-content">Pro plan:</strong> Best for predictable workloads. Pay
                $29/month for 10M KU included, plus $3 per million KU beyond that. Ideal when you know your
                approximate usage.
              </div>
              <div>
                <strong className="text-content">Scale plan:</strong> Pure usage-based pricing. Pay a minimum
                $99/month platform fee plus $2 per million KU consumed (cheaper per-KU rate than Pro). Perfect
                for high-volume or highly variable workloads where you want the lowest per-KU cost without
                worrying about tiers.
              </div>
            </>
          }
        />
        <FaqItem
          question="When does my billing period reset?"
          answer={
            <>
              Your KU usage resets at the start of each billing period. For monthly subscriptions, this is the
              same day each month that you subscribed. For annual subscriptions, usage resets monthly but
              you&apos;re billed annually at a discounted rate (save up to 20%).
            </>
          }
        />
        <FaqItem
          question="Are standard reads really free?"
          answer={
            <>
              Yes! Standard queries and read operations consume zero KU. You only pay for write operations
              (creating or updating records, generating embeddings, forming relationships). Compute-intensive
              operations — vector similarity search, raw Cypher execution, and deep multi-hop traversals —
              consume a small amount of KU because they scale with dataset size rather than data written. A
              small daily storage footprint charge applies to data at rest.
            </>
          }
        />
        <FaqItem
          question="Can I set spending limits to avoid unexpected charges?"
          answer={
            <>
              Yes. In your dashboard, you can set custom KU usage alerts and optional hard spending caps. For
              example, you can configure an alert at 10M KU and a hard cap at 15M KU to prevent overage
              charges. When a hard cap is reached, write operations will be blocked until the next billing
              period (similar to the Free plan behavior).
            </>
          }
        />
        <FaqItem
          question="How do I estimate my KU usage?"
          answer={
            <>
              Use our interactive pricing calculator above! As a rough guide:
              <ul className="mt-2 list-inside list-disc space-y-1">
                <li>A simple record with 10 properties = 10 KU</li>
                <li>Creating 1,000 flat records (10 fields each) = 10,000 KU</li>
                <li>Generating an embedding = 5 KU per record</li>
                <li>Nested objects are decomposed into linked records (each contributes its own KU)</li>
              </ul>
              <div className="mt-3">
                The Free plan (100K KU) typically supports ~3,000 records with 10 fields each. Pro (10M KU)
                supports ~300,000 similar records per month.
              </div>
            </>
          }
        />
        <FaqItem
          question="What if I need more than Scale can offer?"
          answer={
            <>
              BYOC (Bring Your Own Cloud) is available on all plans — simply connect RushDB to your own Neo4j
              or Aura instance. Enterprise adds dedicated infrastructure, embedded/OEM licensing, custom
              contracts, and hands-on deployment support for organisations with compliance requirements or
              very high volume needs. Contact us at{' '}
              <a href="mailto:hi@rushdb.com" className="text-accent hover:underline">
                hi@rushdb.com
              </a>{' '}
              to discuss your requirements.
            </>
          }
        />
      </div>
    </div>
  )
}

export function Pricing({ billingData }: PricingProps) {
  const [annual, setAnnual] = useState(true)

  const proPrice = useMemo(
    () => (annual ? billingData?.pro.annual.amount : billingData?.pro.monthly.amount),
    [annual, billingData]
  )

  return (
    <Section className="container gap-8">
      <SectionHeader className="text-center">
        <SectionTitle className="m-auto max-w-3xl">Simple, Predictable Pricing</SectionTitle>
        <SectionSubtitle className="text-content3">
          Pay for knowledge created — not infrastructure. Start free, scale when ready.
        </SectionSubtitle>
      </SectionHeader>
      <div className="mx-auto mb-4 flex items-center justify-center gap-4">
        <p className="text-content2 typography-base font-medium">Monthly</p>
        <Switch checked={annual} onCheckedChange={(checked) => setAnnual(checked)} />
        <p className="text-content2 typography-base font-medium">
          Annual
          <span className="text-accent-green bg-accent-green/10 ml-2 rounded-full px-2 py-0.5 text-sm font-bold">
            Save up to 20%
          </span>
        </p>
      </div>
      <KuDefinition />
      <div className="mb-8 grid grid-cols-4 gap-4 sm:grid-cols-1 md:grid-cols-2 lg:grid-cols-4">
        <PricingCard
          price="free"
          pricePrefix="forever"
          title="Free"
          description="Perfect for side projects and learning"
          headline="Get started instantly:"
          pricePlaceholder={'FREE'}
          action={
            <Button size="small" variant="outline" as={Link} href={links.app} target="_blank">
              Start Building Free
              <ArrowUpRight />
            </Button>
          }
        >
          <Feat title="100K KU / month" subtitle="~3,000 records with 10 fields each" />
          <Feat title="2 projects" subtitle="No time limits, no feature restrictions" />
          <Feat title="Full REST API and SDKs" subtitle="Complete access to all query capabilities" />{' '}
          <Feat title="Self-hosted &amp; BYOC" subtitle="Connect to your own Neo4j or Aura instance" />{' '}
          <Feat title="Vector & AI search" subtitle="Native support for embeddings and similarity" />
          <Feat title="Community support" subtitle="Get help from our developer community" />
        </PricingCard>

        <PricingCard
          price={proPrice}
          pricePrefix="from"
          title="Pro"
          featured
          description="Perfect for production applications"
          headline="Everything in Free, plus:"
          action={
            <Button size="small" variant="accent" as={Link} href={links.app}>
              Upgrade Now
              <ArrowUpRight />
            </Button>
          }
        >
          <Feat title="10M KU / month" subtitle="~300,000 records with 10 fields each" />
          <Feat title="Overage billing" subtitle="Continue beyond 10M KU at a per-KU rate" />
          <Feat title="Unlimited projects" subtitle="No per-project limits" />
          <Feat title="3 team members" subtitle="then $10 per member" />
          <Feat title="Self-hosted &amp; BYOC" subtitle="Connect to your own Neo4j or Aura instance" />
        </PricingCard>

        <PricingCard
          pricePlaceholder="Usage-based"
          pricePrefix="&nbsp;"
          title="Scale"
          description="For high-volume and data-intensive apps"
          headline="Everything in Pro, plus:"
          action={
            <Button size="small" variant="outline" as={Link} href="mailto:hello@rushdb.com">
              Talk to Us
              <ArrowUpRight />
            </Button>
          }
        >
          <Feat title="Unlimited KU" subtitle="Usage-based billing — pay only for what you consume" />
          <Feat title="Unlimited team members" subtitle="No per-seat limit" />
          <Feat title="Self-hosted &amp; BYOC" subtitle="Connect to your own Neo4j or Aura instance" />
          <Feat title="SLA guarantee" subtitle="Uptime and performance guarantees" />
          <Feat title="Priority support" subtitle="Fast-track access to expert help" />
        </PricingCard>

        <PricingCard
          pricePlaceholder="Contact Us"
          title="Enterprise"
          pricePrefix="&nbsp;"
          description="For organisations and embedded use"
          headline="Everything in Scale, plus:"
          action={
            <Button size="small" variant="outline" as={Link} href="mailto:hi@rushdb.com">
              Contact Us
              <ArrowUpRight />
            </Button>
          }
        >
          <Feat title="Platform license" subtitle="Unlimited KU, flat-fee pricing" />
          <Feat
            title="BYOC — Dedicated deployment"
            subtitle="Managed setup in your private cloud with dedicated support"
          />
          <Feat title="Embedded / OEM use" subtitle="Build RushDB into your own product" />
          <Feat title="Dedicated support" subtitle="Phone, chat, and dedicated account manager" />
        </PricingCard>
      </div>
      <PricingCalculator />
      <PricingComparison className="mt-16" />
      <PricingFaq />
    </Section>
  )
}
