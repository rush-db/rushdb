import { MemoryStick, Cpu, Zap, Database } from 'lucide-react'
import React, { useState, useCallback, useMemo, useEffect, useRef } from 'react'
import NumberFlow from '@number-flow/react'
import { CodeBlock } from './CodeBlock'
import { TieredPricingData } from './pricing-types'

interface PricingCalculatorProps {
  tieredPricingData: TieredPricingData
  onTierChange?: (recordsCount: number) => void
  billingType: 'onDemand' | 'reserved'
}

const generateSampleRecord = (propertyCount: number) => {
  const baseProperties = {
    name: 'John Smith'
  }

  const additionalProperties: Record<string, any> = {
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
    browser: 'Safari',
    operating_system: 'iOS 17.2',
    preferred_contact_time: 'morning',
    preferred_contact_method: 'email'
  }

  // Start with base properties (always include these)
  const result: Record<string, any> = { ...baseProperties }

  // Add additional properties up to the specified count
  const additionalKeys = Object.keys(additionalProperties)
  const numAdditional = Math.max(0, propertyCount - Object.keys(baseProperties).length)

  for (let i = 0; i < Math.min(numAdditional, additionalKeys.length); i++) {
    const key = additionalKeys[i]
    result[key] = additionalProperties[key]
  }

  return result
}

export function PricingCalculator({ tieredPricingData, onTierChange, billingType }: PricingCalculatorProps) {
  const [recordsCount, setRecordsCount] = useState(10000)
  const [avgProperties, setAvgProperties] = useState(5)
  const leftBlockRef = useRef<HTMLDivElement>(null)
  const [leftBlockHeight, setLeftBlockHeight] = useState<number | undefined>(undefined)

  const sampleRecord = useMemo(() => generateSampleRecord(avgProperties), [avgProperties])

  function getTierByPageCache(recordsCount: number, avgProperties: number) {
    const recordRAM = 15 + avgProperties * 75 + ((avgProperties * 7) / 13) * 128 // bytes

    for (const option of tieredPricingData) {
      const pageCacheGB = option.ram / 2
      const pageCacheBytes = pageCacheGB * 1_000_000_000
      const maxRecordsInCache = pageCacheBytes / recordRAM
      const ratio = maxRecordsInCache / recordsCount
      if (ratio >= 0.1) {
        return {
          instanceRAM: option.ram,
          tier: option.tier,
          instanceCPU: option.cpu,
          instanceDisk: option.disk,
          pageCacheGB,
          recordRAM,
          ratio: Math.min(ratio, 1)
        }
      }
    }
    // fallback to largest instance
    const last = tieredPricingData[tieredPricingData.length - 1]
    const pageCacheGB = last.ram / 2
    const pageCacheBytes = pageCacheGB * 1_000_000_000
    const maxRecordsInCache = pageCacheBytes / recordRAM
    const ratio = maxRecordsInCache / recordsCount
    return {
      tier: last.tier,
      instanceRAM: last.ram,
      instanceCPU: last.cpu,
      instanceDisk: last.disk,
      pageCacheGB,
      recordRAM,
      ratio: Math.min(ratio, 1)
    }
  }

  useEffect(() => {
    onTierChange?.(recordsCount)
  }, [recordsCount, onTierChange])

  const handleRecordsSliderChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const sliderValue = Number(e.target.value)
    const actualRecords = sliderToRecords(sliderValue)
    setRecordsCount(actualRecords)

    onTierChange?.(actualRecords)
  }

  const pricingInfo = useMemo(
    () => getTierByPageCache(recordsCount, avgProperties),
    [recordsCount, avgProperties]
  )

  const calculatePricing = useCallback(() => {
    const [tierLevel, subTier] = pricingInfo.tier.split('-')
    const tierData = tieredPricingData.find((t) => t.tier === pricingInfo.tier)!
    return {
      tierLevel,
      subTier,
      onDemandPrice: tierData.onDemand.amount,
      price: billingType === 'onDemand' ? tierData.onDemand : tierData.reserved.amount,
      discount:
        billingType === 'reserved' ?
          Math.round(((tierData.onDemand.amount - tierData.reserved.amount) / tierData.onDemand.amount) * 100)
        : 0
    }
  }, [pricingInfo, avgProperties, billingType, tieredPricingData])

  const pricing = calculatePricing()

  const formatRecordsCount = (count: number) => {
    if (count >= 1000000000) return `${(count / 1000000000).toFixed(1)}B`
    if (count >= 1000000) return `${(count / 1000000).toFixed(1)}M`
    if (count >= 1000) return `${(count / 1000).toFixed(1)}K`
    return count.toString()
  }

  const scalePoints = [
    { value: 1000, position: 0 },
    { value: 10000, position: 11.5 },
    { value: 100000, position: 23.5 },
    { value: 1000000, position: 36.5 },
    { value: 10000000, position: 48 },
    { value: 50000000, position: 60.5 },
    { value: 100000000, position: 73.5 },
    { value: 500000000, position: 87 },
    { value: 1000000000, position: 100 }
  ]

  const snapToClosestPoint = (position: number) => {
    let closestPoint = scalePoints[0]
    let minDistance = Math.abs(position - closestPoint.position)

    for (const point of scalePoints) {
      const distance = Math.abs(position - point.position)
      if (distance < minDistance) {
        minDistance = distance
        closestPoint = point
      }
    }

    return closestPoint
  }

  const sliderToRecords = (position: number) => {
    const snappedPoint = snapToClosestPoint(position)
    return snappedPoint.value
  }

  const recordsToSlider = (records: number) => {
    const point = scalePoints.find((p) => p.value === records)
    return point ? point.position : scalePoints[0].position
  }

  useEffect(() => {
    if (leftBlockRef.current) {
      setLeftBlockHeight(leftBlockRef.current.offsetHeight)
    }
  }, [recordsCount, avgProperties, pricing, pricingInfo])

  return (
    <div className="grid grid-cols-2 gap-8 md:grid-cols-1">
      <div className="" ref={leftBlockRef}>
        <div className="mb-6">
          <h3 className="typography-2xl text-content">Find Your Perfect Plan</h3>
          <p className="text-content3 mt-2 text-sm font-medium">
            This calculator provides cost estimates based on your expected usage:
          </p>
          <ul className="text-content3 mt-2 space-y-1 text-sm font-medium">
            <li>• 1-click scale up/down on demand</li>
            <li>• Scale at your convenience with accurate usage stats</li>
            <li>• No hidden fees, no autoscaling traps, transparent pricing</li>
          </ul>
        </div>

        <div className="mb-8">
          <label className="text-content2 typography-base mb-4 block font-medium">
            Records Count: <span className="text-accent font-bold">{formatRecordsCount(recordsCount)}</span>
          </label>
          <div className="relative">
            <input
              type="range"
              min="0"
              max="100"
              step="0.1"
              value={recordsToSlider(recordsCount)}
              onChange={handleRecordsSliderChange}
              className="records-slider bg-fill2 h-3 w-full cursor-pointer appearance-none rounded-lg"
              style={{
                background: `linear-gradient(to right, #3f81ff 0%, #3f81ff ${recordsToSlider(recordsCount)}%, hsl(var(--color-fill2)) ${recordsToSlider(recordsCount)}%, hsl(var(--color-fill2)) 100%)`
              }}
            />
            <div className="text-content3 mt-2 flex justify-between px-1 text-xs">
              <span className="font-bold">1K</span>
              <span>10K</span>
              <span className="font-bold">100K</span>
              <span>1M</span>
              <span className="font-bold">10M</span>
              <span>50M</span>
              <span className="font-bold">100M</span>
              <span>500M</span>
              <span className="font-bold">1B</span>
            </div>
          </div>
          <div className="text-content3 mt-1 flex justify-between text-xs">
            <span className="font-medium">T3 Tier (Up to 10M)</span>
            <span className="font-medium">T2 Tier (Up to 100M)</span>
            <span className="font-medium">T1 Tier (100M+)</span>
          </div>
        </div>

        {/* Average Properties Slider */}
        <div className="mb-8">
          <label className="text-content2 typography-base mb-4 block font-medium">
            Average Properties per Record: <span className="text-accent font-bold">{avgProperties}</span>
          </label>
          <div className="relative">
            <input
              type="range"
              min="1"
              max="50"
              step="1"
              value={avgProperties}
              onChange={(e) => setAvgProperties(Number(e.target.value))}
              className="properties-slider bg-fill2 h-3 w-full cursor-pointer appearance-none rounded-lg"
              style={{
                background: `linear-gradient(to right, #3f81ff 0%, #3f81ff ${((avgProperties - 1) / 49.5) * 100}%, hsl(var(--color-fill2)) ${((avgProperties - 1) / 49.5) * 100}%, hsl(var(--color-fill2)) 100%)`
              }}
            />
            <div className="text-content3 mt-2 flex justify-between text-xs">
              <span className="font-bold">1</span>
              <span>5</span>
              <span className="font-bold">10</span>
              <span>15</span>
              <span className="font-bold">20</span>
              <span>25</span>
              <span className="font-bold">30</span>
              <span>35</span>
              <span className="font-bold">40</span>
              <span>45</span>
              <span className="font-bold">50</span>
            </div>
          </div>
          <div className="text-content3 mt-1 flex justify-between text-xs">
            <span className="font-medium">Sub-tier C (≤10)</span>
            <span className="font-medium">Sub-tier B (11-30)</span>
            <span className="font-medium">Sub-tier A (31-50)</span>
          </div>
        </div>

        {/* Pricing Result */}
        <div className="bg-accent/5 border-accent rounded-lg border p-6">
          <div className="flex items-start justify-between">
            <div>
              <p className="text-content3 mb-2 text-sm">Estimated Configuration</p>
              <p className="text-content typography-2xl mb-2 font-bold">Tier {pricingInfo.tier}</p>
            </div>
            <div className="text-right">
              <p className="text-content3 text-sm">Price per month</p>
              <div className="flex items-baseline justify-end">
                {billingType === 'reserved' && pricing.discount > 0 && (
                  <span className="text-content3 relative mr-2 inline-block text-lg">
                    <NumberFlow value={pricing.onDemandPrice} decimals={0} prefix="$" duration={1} />
                    <span className="pointer-events-none absolute left-0 right-0 top-1/2 h-[1px] bg-current" />
                  </span>
                )}
                <span className="text-accent typography-2xl font-bold">
                  <NumberFlow value={pricing.price} decimals={0} prefix="$" duration={1} />
                </span>
              </div>
              {pricing.discount > 0 && (
                <p className="text-accent-green text-sm font-medium">
                  Save <NumberFlow value={pricing.discount} decimals={0} suffix="%" duration={1} /> with
                  1-year commitment
                </p>
              )}
            </div>
          </div>
          <div className="text-md mt-8 flex items-center justify-between gap-4 font-bold">
            <span className="text-content3 flex items-center gap-1">
              <Cpu className="text-accent h-5 w-5" />
              <NumberFlow value={pricingInfo.instanceCPU} decimals={0} duration={1} />{' '}
              <span className="mb-0.5 self-end text-sm font-medium">CPU</span>
            </span>
            <span className="text-content3 flex items-center gap-1">
              <Database className="text-accent h-5 w-5" />
              <NumberFlow value={pricingInfo.instanceDisk} decimals={0} duration={1} />{' '}
              <span className="mb-0.5 self-end text-sm font-medium">GB SSD</span>
            </span>
            <span className="text-content3 flex items-center gap-1">
              <MemoryStick className="text-accent h-5 w-5" />
              <NumberFlow value={pricingInfo.instanceRAM} decimals={0} duration={1} />{' '}
              <span className="mb-0.5 self-end text-sm font-medium">GB RAM</span>
            </span>
            <span className="text-content3 flex items-center gap-1">
              <Zap className="text-accent h-5 w-5" />
              <NumberFlow value={pricingInfo.pageCacheGB} decimals={0} duration={1} />{' '}
              <span className="mb-0.5 self-end text-sm font-medium">GB Live Query Cache</span>
            </span>
          </div>
        </div>

        <div className="mt-4 rounded-lg p-3">
          <p className="text-content3 text-sm font-medium">
            <strong>Disclaimer:</strong> Performance depends on data complexity{' '}
            <strong>(average properties per record)</strong>, total record count, and data diversity. This
            calculator provides an overview of recommended tiers tailored to typical workloads. Your specific
            use case may not require an upgrade, or may need to scale up sooner, depending on these factors.
          </p>
        </div>
      </div>
      <CodeBlock
        language="json"
        className="overflow-y overflow-x-hidden rounded-lg"
        style={leftBlockHeight ? { maxHeight: leftBlockHeight } : undefined}
        code={`// Sample User Record with ${avgProperties} properties.\n// Adjust the slider to see how complexity affects pricing.\n${JSON.stringify(sampleRecord, null, 2)}`}
      />
    </div>
  )
}
