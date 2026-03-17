import React, { useEffect, useMemo, useRef, useState } from 'react'
import NumberFlow from '@number-flow/react'
import { CodeBlock } from '~/components/CodeBlock'

interface PricingCalculatorProps {
  cta?: (planId: string) => React.ReactNode
}

const PLANS = [
  { id: 'free', label: 'Free', kuIncluded: 100_000, price: 0 },
  { id: 'pro', label: 'Pro', kuIncluded: 10_000_000, price: 29 },
  { id: 'scale', label: 'Scale', kuIncluded: null, price: null }
]

// Canonical sample record — slider controls how many properties are visible
const SAMPLE_RECORD: Record<string, string | number | boolean | string[]> = {
  name: 'John Smith',
  id: 'usr_2024_001',
  email: 'john.smith@example.com',
  age: 28,
  city: 'San Francisco',
  country: 'USA',
  occupation: 'Software Engineer',
  company: 'TechCorp Inc',
  salary: 125000,
  experience_years: 5,
  skills: ['JavaScript', 'Python', 'React'],
  last_login: '2024-01-15T10:30:00Z',
  subscription_tier: 'premium',
  account_created: '2023-06-01T00:00:00Z',
  phone: '+1-555-0123',
  address: '123 Main St, Apt 4B',
  zip_code: '94102',
  timezone: 'America/Los_Angeles',
  language_preference: 'en-US',
  email_notifications: true,
  sms_notifications: false,
  push_notifications: true,
  profile_picture: '/avatars/john.jpg',
  linkedin_url: 'https://linkedin.com/in/johnsmith',
  twitter_handle: '@johnsmith',
  theme_preference: 'dark',
  newsletter_subscribed: true,
  traffic_source: 'organic_search',
  campaign_name: 'winter_2023',
  last_purchase_date: '2023-12-01',
  last_purchase_amount: 299.99,
  last_product: 'Premium Plan',
  activity_score: 85.7,
  tags: ['high_value', 'active_user', 'developer'],
  referral_code: 'REF2024JS',
  two_factor_enabled: true,
  profile_public: false,
  show_activity: true,
  emergency_contact_name: 'Jane Smith',
  emergency_contact_relation: 'spouse',
  emergency_contact_phone: '+1-555-0124',
  work_timezone: 'PST',
  work_hours: '9AM-5PM',
  work_days: ['monday', 'tuesday', 'wednesday', 'thursday', 'friday'],
  certifications: ['AWS Solutions Architect', 'Google Cloud Professional'],
  interests: ['technology', 'hiking', 'photography', 'cooking'],
  fitness_goal: 'weight_loss',
  activity_level: 'moderate',
  primary_device: 'iPhone 15 Pro',
  browser: 'Safari'
}

function getNestingFactor(depth: number): number {
  return Math.min(1 + (depth - 1) * 0.5, 3.0)
}

function getRecommendedPlan(monthlyKu: number): (typeof PLANS)[0] {
  if (monthlyKu <= 100_000) return PLANS[0]
  if (monthlyKu <= 10_000_000) return PLANS[1]
  return PLANS[2]
}

function formatKu(ku: number): string {
  if (ku >= 1_000_000) return `${(ku / 1_000_000).toFixed(1)}M`
  if (ku >= 1_000) return `${(ku / 1_000).toFixed(0)}K`
  return ku.toString()
}

export function PricingCalculator({ cta }: PricingCalculatorProps) {
  const [recordsPerDay, setRecordsPerDay] = useState(500)
  const [avgFields, setAvgFields] = useState(8)
  const [nestingDepth, setNestingDepth] = useState(1)

  const estimatedMonthlyKu = useMemo(() => {
    return Math.round(recordsPerDay * 30 * avgFields * getNestingFactor(nestingDepth))
  }, [recordsPerDay, avgFields, nestingDepth])

  const kuPerRecord = useMemo(
    () => Math.round(avgFields * getNestingFactor(nestingDepth)),
    [avgFields, nestingDepth]
  )

  const recommendedPlan = useMemo(() => getRecommendedPlan(estimatedMonthlyKu), [estimatedMonthlyKu])

  const sampleRecord = useMemo(() => {
    const keys = Object.keys(SAMPLE_RECORD).slice(0, Math.min(avgFields, Object.keys(SAMPLE_RECORD).length))
    return Object.fromEntries(keys.map((k) => [k, SAMPLE_RECORD[k]]))
  }, [avgFields])

  const leftColRef = useRef<HTMLDivElement>(null)
  const [leftBlockHeight, setLeftBlockHeight] = useState<number | undefined>()

  useEffect(() => {
    const el = leftColRef.current
    if (!el) return
    const ro = new ResizeObserver(() => setLeftBlockHeight(el.offsetHeight))
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  const sliderBg = (value: number, min: number, max: number, color = '#3f81ff') => {
    const pct = ((value - min) / (max - min)) * 100
    return `linear-gradient(to right, ${color} 0%, ${color} ${pct}%, hsl(var(--color-fill2)) ${pct}%, hsl(var(--color-fill2)) 100%)`
  }

  return (
    <div className="w-full">
      <div className="grid grid-cols-2 items-start gap-10 md:grid-cols-1">
        <div ref={leftColRef} className="flex flex-col gap-8">
          <div>
            <h3 className="typography-xl text-content mb-1 font-bold">Estimate Your Usage</h3>
            <p className="text-content3 mb-8 text-sm font-medium">
              Adjust the sliders to see which plan fits your workload.
            </p>
          </div>
          <div>
            <label className="text-content2 typography-base mb-3 block font-medium">
              Records ingested per day:{' '}
              <span className="text-accent font-bold">{recordsPerDay.toLocaleString()}</span>
            </label>
            <input
              type="range"
              min={10}
              max={100_000}
              step={10}
              value={recordsPerDay}
              onChange={(e) => setRecordsPerDay(Number(e.target.value))}
              className="h-3 w-full cursor-pointer appearance-none rounded-lg"
              style={{ background: sliderBg(recordsPerDay, 10, 100_000) }}
            />
            <div className="text-content3 mt-1 flex justify-between text-xs font-medium">
              <span>10</span>
              <span>1K</span>
              <span>10K</span>
              <span>100K</span>
            </div>
          </div>

          <div>
            <label className="text-content2 typography-base mb-3 block font-medium">
              Avg properties per record: <span className="text-accent font-bold">{avgFields}</span>
            </label>
            <input
              type="range"
              min={1}
              max={50}
              step={1}
              value={avgFields}
              onChange={(e) => setAvgFields(Number(e.target.value))}
              className="h-3 w-full cursor-pointer appearance-none rounded-lg"
              style={{ background: sliderBg(avgFields, 1, 50) }}
            />
            <div className="text-content3 mt-1 flex justify-between text-xs font-medium">
              <span>1</span>
              <span>10</span>
              <span>25</span>
              <span>50</span>
            </div>
          </div>

          <div>
            <label className="text-content2 typography-base mb-1 block font-medium">
              Average nesting depth: <span className="text-accent font-bold">{nestingDepth}</span>
            </label>
            <p className="text-content3 mb-3 text-xs font-medium">
              Deeper nesting creates more child records and relationships — each billed independently.
            </p>
            <input
              type="range"
              min={1}
              max={5}
              step={1}
              value={nestingDepth}
              onChange={(e) => setNestingDepth(Number(e.target.value))}
              className="h-3 w-full cursor-pointer appearance-none rounded-lg"
              style={{ background: sliderBg(nestingDepth, 1, 5) }}
            />
            <div className="text-content3 mt-1 flex justify-between text-xs font-medium">
              <span>Flat (1)</span>
              <span>2</span>
              <span>3</span>
              <span>4</span>
              <span>Deep (5)</span>
            </div>
          </div>
          <div className="bg-accent/5 border-accent rounded-xl border p-5">
            <div className="mb-4">
              <p className="text-content3 mb-1 text-sm font-medium">Estimated monthly usage</p>
              <p className="typography-3xl text-accent font-bold">
                <NumberFlow value={estimatedMonthlyKu} duration={0.6} /> KU
              </p>
            </div>
            <div>
              <p className="text-content2 mb-2 text-sm font-medium">Recommended plan</p>
              <div className="flex flex-col gap-2">
                {PLANS.map((plan) => {
                  const isRecommended = plan.id === recommendedPlan.id
                  return (
                    <div
                      key={plan.id}
                      className={`flex items-center justify-between rounded-lg px-3 py-2 transition-all ${
                        isRecommended ? 'bg-accent text-white' : 'bg-fill2 text-content3'
                      }`}
                    >
                      <span className="text-sm font-semibold">{plan.label}</span>
                      <span className="text-xs font-medium">
                        {plan.kuIncluded ? `${formatKu(plan.kuIncluded)} KU` : 'Unlimited KU'}
                        {plan.price !== null ? ` — $${plan.price}/mo` : ' — usage-based'}
                      </span>
                    </div>
                  )
                })}
              </div>
            </div>
            {cta && <div className="mt-4">{cta(recommendedPlan.id)}</div>}
          </div>
        </div>

        <CodeBlock
          language="json"
          className="overflow-y overflow-x-hidden rounded-lg"
          style={leftBlockHeight ? { maxHeight: leftBlockHeight } : undefined}
          code={`// Sample record with ${avgFields} properties.\n// Properties written to each record consume KU.\n// Nested objects become separate linked records.\n${JSON.stringify(sampleRecord, null, 2)}`}
        />
      </div>
    </div>
  )
}
