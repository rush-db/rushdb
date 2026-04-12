import { useMemo, useState, ReactNode } from 'react'
import { motion } from 'framer-motion'
import Link from 'next/link'
import { Check } from 'lucide-react'
import { FaqItem } from '~/components/Faq'
import { KuDefinition } from '~/components/KuDefinition'
import NumberFlow from '@number-flow/react'
import { links } from '~/config/urls'
import { BillingData } from '~/components/pricing-types'
import {
  LPSection,
  LPContainer,
  LPEyebrow,
  LPSectionHeading,
  LPPrimaryBtn,
  LPGhostBtn,
  LPSwitch,
  fadeUp
} from '~/components/lp/ui'

interface LPPricingProps {
  billingData?: BillingData | null
  showEnterprise?: boolean
  showKuDefinition?: boolean
}

// ── Single plan card ────────────────────────────────────────────────────────
function PlanCard({
  name,
  pricePrefix,
  price,
  pricePlaceholder,
  description,
  headline,
  features,
  cta,
  href,
  highlight,
  delay = 0
}: {
  name: string
  pricePrefix?: string
  price?: number | 'free'
  pricePlaceholder?: string
  description: string
  headline: string
  features: { title: ReactNode; sub?: ReactNode }[]
  cta: string
  href: string
  highlight?: boolean
  delay?: number
}) {
  return (
    <motion.div
      {...fadeUp(delay)}
      className="flex flex-col gap-6 border p-8"
      style={{
        borderColor: highlight ? 'var(--lp-accent)' : 'var(--lp-border)',
        background: highlight ? 'rgba(0,255,133,0.04)' : 'transparent'
      }}
    >
      {/* price header */}
      <div>
        <p className="text-lp-muted mb-3 font-mono text-sm uppercase tracking-widest">{name}</p>
        <div className="mb-1 flex items-baseline gap-1">
          {typeof price === 'number' ?
            <>
              <span className="text-lp-muted font-mono text-sm">{pricePrefix}</span>
              <span className="text-lp-text font-mono text-4xl font-bold">
                <NumberFlow value={price} decimals={0} prefix="$" duration={1} />
              </span>
              <span className="text-lp-muted font-mono text-sm">/ mo</span>
            </>
          : price === 'free' ?
            <span className="text-lp-text font-mono text-4xl font-bold">FREE</span>
          : <span className="text-lp-text font-mono text-4xl font-bold">{pricePlaceholder ?? 'Custom'}</span>}
        </div>
        <p className="text-lp-muted font-mono text-sm">{description}</p>
      </div>

      <div style={{ height: 1, background: 'var(--lp-border)' }} />

      {/* features */}
      <div>
        <p className="text-lp-muted mb-3 font-mono text-sm uppercase tracking-widest">{headline}</p>
        <ul className="flex flex-col gap-3">
          {features.map(({ title, sub }, i) => (
            <li key={i} className="font-mono text-sm">
              <div className="flex items-start gap-2">
                <Check className="mt-0.5 h-3.5 w-3.5 shrink-0" style={{ color: 'var(--lp-accent)' }} />
                <span className="text-lp-text">{title}</span>
              </div>
              {sub && <p className="text-lp-muted pl-5.5 mt-0.5 text-sm">{sub}</p>}
            </li>
          ))}
        </ul>
      </div>

      <div className="mt-auto pt-2">
        {highlight ?
          <LPPrimaryBtn href={href} className="w-full text-center">
            {cta}
          </LPPrimaryBtn>
        : <LPGhostBtn href={href} className="w-full text-center">
            {cta}
          </LPGhostBtn>
        }
      </div>
    </motion.div>
  )
}

// ── Toggle (annual/monthly) ─────────────────────────────────────────────────
function Toggle({ on, onToggle }: { on: boolean; onToggle: () => void }) {
  return (
    <span className="flex items-center gap-3">
      <span className="text-lp-muted font-mono text-sm" style={{ color: !on ? 'var(--lp-text)' : undefined }}>
        Monthly
      </span>
      <LPSwitch checked={on} onCheckedChange={onToggle} aria-label="Toggle billing period" />
      <span className="text-lp-muted font-mono text-sm" style={{ color: on ? 'var(--lp-text)' : undefined }}>
        Annual{' '}
      </span>
    </span>
  )
}

// ── Main export ─────────────────────────────────────────────────────────────
export function LPPricing({ billingData, showEnterprise = true, showKuDefinition = false }: LPPricingProps) {
  const [annual, setAnnual] = useState(true)

  const proPrice = useMemo(
    () => (annual ? billingData?.pro.annual.amount : billingData?.pro.monthly.amount) ?? (annual ? 25 : 29),
    [annual, billingData]
  )

  const scalePrice = useMemo(
    () =>
      (annual ? billingData?.scale?.annual.amount : billingData?.scale?.monthly.amount) ?? (annual ? 89 : 99),
    [annual, billingData]
  )

  return (
    <LPSection surface borderY>
      <LPContainer size="lg">
        <LPEyebrow>Pricing</LPEyebrow>
        <LPSectionHeading className="mb-6">Simple, predictable pricing.</LPSectionHeading>
        <p className="text-lp-muted mb-10 font-mono text-sm">
          Pay for knowledge created — not infrastructure. Standard reads are always free.{' '}
          {!showKuDefinition && (
            <Link href={links.pricing} className="text-lp-accent hover:underline">
              What is a KU? →
            </Link>
          )}
        </p>

        {showKuDefinition && <KuDefinition variant="lp" />}

        <div className="m-auto mb-12 flex w-full items-center justify-center">
          <Toggle on={annual} onToggle={() => setAnnual((v) => !v)} />
        </div>

        <div className="mb-8 grid grid-cols-3 gap-px sm:grid-cols-1 md:grid-cols-2">
          <div style={{ background: 'var(--lp-surface)' }}>
            <PlanCard
              name="Free"
              price="free"
              description="Perfect for side projects and learning"
              headline="Get started instantly:"
              delay={0}
              features={[
                { title: '100K KU / month', sub: '~3,000 records with 10 fields each' },
                { title: '2 projects', sub: 'No time limits, no feature restrictions' },
                { title: 'Full REST API and SDKs' },
                { title: 'Self-hosted & BYOC', sub: 'Connect to your own Neo4j or Aura' },
                { title: 'Vector & AI search' },
                { title: 'Community support' }
              ]}
              cta="Start Building Free"
              href={links.app}
            />
          </div>
          <div style={{ background: 'var(--lp-surface)' }}>
            <PlanCard
              name="Pro"
              pricePrefix="from"
              price={proPrice}
              description="Perfect for production applications"
              headline="Everything in Free, plus:"
              highlight
              delay={0.05}
              features={[
                { title: '10M KU / month', sub: '~300,000 records with 10 fields each' },
                { title: 'Overage billing', sub: 'Continue beyond 10M KU at $3 / M KU' },
                { title: 'Unlimited projects' },
                { title: '3 team members', sub: 'then $10 / seat' },
                { title: 'Self-hosted & BYOC' }
              ]}
              cta="Upgrade Now"
              href={links.app}
            />
          </div>
          <div style={{ background: 'var(--lp-surface)' }}>
            <PlanCard
              name="Scale"
              pricePrefix="from"
              price={scalePrice}
              pricePlaceholder="Usage-based"
              description="For high-volume and data-intensive apps"
              headline="Everything in Pro, plus:"
              delay={0.1}
              features={[
                { title: 'Unlimited KU', sub: 'Usage-based — pay only for what you consume' },
                { title: 'Unlimited team members' },
                { title: 'Self-hosted & BYOC' },
                { title: 'SLA guarantee' },
                { title: 'Priority support' }
              ]}
              cta="Upgrade"
              href={links.appPricing}
            />
          </div>
        </div>

        {/* Enterprise — full-width banner */}
        {showEnterprise && (
          <motion.div
            {...fadeUp(0.15)}
            className="mb-8 border p-8"
            style={{ borderColor: 'var(--lp-border)' }}
          >
            <div className="flex items-start justify-between gap-12 md:flex-col md:gap-6">
              <div className="max-w-sm shrink-0">
                <p className="text-lp-muted mb-2 font-mono text-sm uppercase tracking-widest">Enterprise</p>
                <p className="text-lp-text mb-3 font-mono text-2xl font-bold">
                  For organisations &amp; embedded use
                </p>
                <p className="text-lp-muted font-mono text-sm leading-relaxed">
                  Dedicated infrastructure, embedded/OEM licensing, custom contracts, and hands-on deployment
                  support for compliance-heavy or high-volume needs.
                </p>
              </div>
              <div className="flex flex-1 flex-col justify-between gap-6 md:w-full">
                <div
                  className="grid grid-cols-2 gap-x-8 gap-y-3 font-mono text-sm sm:grid-cols-1"
                  style={{ color: '#6B6B72' }}
                >
                  {[
                    'Platform license',
                    'Dedicated BYOC deployment',
                    'Embedded / OEM use',
                    'Dedicated support'
                  ].map((f) => (
                    <span key={f} className="flex items-center gap-1.5">
                      <Check className="h-3 w-3 shrink-0" style={{ color: 'var(--lp-accent)' }} />
                      {f}
                    </span>
                  ))}
                </div>
                <div>
                  <LPGhostBtn href="mailto:hi@rushdb.com" className="md:w-full md:text-center">
                    Contact Us →
                  </LPGhostBtn>
                </div>
              </div>
            </div>
          </motion.div>
        )}

        {/* self-host note */}
        <motion.p {...fadeUp(0.2)} className="text-lp-muted mb-20 font-mono text-sm">
          Or{' '}
          <Link
            href="https://docs.rushdb.com/get-started/self-hosting"
            className="text-lp-accent hover:underline"
          >
            self-host for free →
          </Link>
        </motion.p>

        {/* FAQ */}
        <motion.div {...fadeUp(0.1)}>
          <p className="text-lp-muted mb-6 font-mono text-sm uppercase tracking-widest">FAQ</p>
          <p className="text-lp-text mb-8 font-mono text-2xl font-bold">Frequently asked questions</p>
          <div style={{ borderTop: '1px solid var(--lp-border)' }}>
            <FaqItem
              question="What happens when I hit my KU limit on the Free plan?"
              answer="On the Free plan (100K KU/month), once you reach your limit, writes pause until your next billing period — reads and standard queries always continue. We'll send email notifications at 75%, 90%, and 100% usage."
            />
            <FaqItem
              question="How does overage billing work on the Pro plan?"
              answer={
                <>
                  Pro includes 10M KU/month for{' '}
                  <span className="text-lp-text font-bold">${annual ? '25' : '29'}/month</span>. Beyond that,
                  you're charged <span className="text-lp-text font-bold">$3 per additional million KU</span>.
                  No hard cap — your app keeps running. Example: 12M KU in a month = ${annual ? '25' : '29'}{' '}
                  base + (2M × $3) = ${annual ? '$31' : '$35'} total.
                </>
              }
            />
            <FaqItem
              question="What's the difference between Pro and Scale?"
              answer={
                <>
                  <strong className="text-lp-text">Pro:</strong> Best for predictable workloads. $
                  {annual ? '25' : '29'}/mo for 10M KU, plus $3/M beyond.{' '}
                  <strong className="text-lp-text">Scale:</strong> Pure usage-based. ${annual ? '89' : '99'}
                  /mo platform fee + $2/M KU — cheaper per-KU rate for high or variable volume.
                </>
              }
            />
            <FaqItem
              question="Are standard reads really free?"
              answer="Yes. Standard queries and reads consume zero KU. You pay only for writes. Compute-intensive operations — vector similarity search, raw Cypher, and deep multi-hop traversals — consume a small amount of KU because they scale with dataset size."
            />
            <FaqItem
              question="How do I estimate my KU usage?"
              answer={
                <>
                  As a rough guide: a record with 10 properties ≈ 10 KU (0.5 base + 1 per property). Creating
                  1,000 flat records (10 fields each) ≈ 10,000 KU. Generating an embedding costs 5 KU per
                  record. The Free plan (100K KU) typically supports ~3,000 such records per month, or ~1,000
                  records with embeddings. Pro (10M KU) supports ~300,000 flat records/month.
                </>
              }
            />
            <FaqItem
              question="What about self-hosting or enterprise licensing?"
              answer={
                <>
                  BYOC (Bring Your Own Cloud) is available on all plans — connect RushDB to your own Neo4j or
                  Aura instance. Enterprise adds dedicated infrastructure, embedded/OEM licensing, and custom
                  contracts.{' '}
                  <a href="mailto:hi@rushdb.com" style={{ color: 'var(--lp-accent)' }}>
                    hi@rushdb.com
                  </a>
                </>
              }
            />
          </div>
        </motion.div>
      </LPContainer>
    </LPSection>
  )
}
