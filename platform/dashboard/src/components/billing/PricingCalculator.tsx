import { useStore } from '@nanostores/react'
import { $currentPeriod, $currentTierIndex, $pricingData } from '~/features/billing/stores/plans.ts'
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import NumberFlow from '@number-flow/react'
import { Cpu, Database, MemoryStick, Zap } from 'lucide-react'

export function PricingCalculator({ cta }: { cta?: React.ReactNode }) {
  const [recordsCount, setRecordsCount] = useState(10000)
  const [avgProperties, setAvgProperties] = useState(5)
  const leftBlockRef = useRef<HTMLDivElement>(null)
  const currentPeriod = useStore($currentPeriod)

  const data = useStore($pricingData)

  const tieredPricingData = data.data?.team ?? []

  function getTierByUsage(recordsCount: number, avgProperties: number) {
    const recordRAM = 15 + avgProperties * 75 + ((avgProperties * 7) / 13) * 128 // bytes
    for (const option of tieredPricingData) {
      const pageCacheGB = option.ram / 2
      const pageCacheBytes = pageCacheGB * 1_000_000_000
      const maxRecordsInCache = pageCacheBytes / recordRAM
      const ratio = maxRecordsInCache / recordsCount
      if (ratio >= 0.15) {
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

  const handleRecordsSliderChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const sliderValue = Number(e.target.value)
    const actualRecords = sliderToRecords(sliderValue)
    setRecordsCount(actualRecords)
  }

  const pricingInfo = useMemo(
    () => getTierByUsage(recordsCount, avgProperties),
    [recordsCount, avgProperties]
  )

  useEffect(() => {
    $currentTierIndex.set(tieredPricingData.findIndex((t) => t.tier === pricingInfo.tier))
  }, [pricingInfo])

  const calculatePricing = useCallback(() => {
    const [tierLevel, subTier] = pricingInfo.tier.split('-')
    const tierData = tieredPricingData.find((t) => t.tier === pricingInfo.tier)!
    return {
      tierLevel,
      subTier,
      onDemandPrice: tierData.onDemand.amount,
      price: currentPeriod === 'month' ? tierData.onDemand.amount : tierData.reserved.amount,
      discount:
        currentPeriod === 'annual' ?
          Math.round(((tierData.onDemand.amount - tierData.reserved.amount) / tierData.onDemand.amount) * 100)
        : 0
    }
  }, [pricingInfo, avgProperties, currentPeriod, tieredPricingData])

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

  return (
    <div className="grid grid-cols-1 gap-8 md:grid-cols-2">
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
                background: `linear-gradient(to right, hsl(72.96 82.69% 61.55% / var(--tw-text-opacity, 1)) 0%, hsl(72.96 82.69% 61.55% / var(--tw-text-opacity, 1)) ${recordsToSlider(recordsCount)}%, #1d1d1d ${recordsToSlider(recordsCount)}%, #1d1d1d 100%)`
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
            <span className="font-medium">T3 Tier</span>
            <span className="font-medium">T2 Tier</span>
            <span className="font-medium">T1 Tier</span>
          </div>
        </div>
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
                background: `linear-gradient(to right, hsl(72.96 82.69% 61.55% / var(--tw-text-opacity, 1)) 0%, hsl(72.96 82.69% 61.55% / var(--tw-text-opacity, 1)) ${((avgProperties - 1) / 49.5) * 100}%, #1d1d1d ${((avgProperties - 1) / 49.5) * 100}%, #1d1d1d 100%)`
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
      </div>
      <div>
        <div className="bg-accent/5 border-accent rounded-lg border p-6">
          <div className="flex items-start justify-between">
            <div>
              <p className="text-content3 mb-2 text-sm">Estimated Configuration</p>
              <p className="text-content typography-2xl mb-2 font-bold">Tier {pricingInfo.tier}</p>
            </div>
            <div className="text-right">
              <p className="text-content3 text-sm">Price per month</p>
              <div className="flex items-baseline justify-end">
                {currentPeriod === 'annual' && pricing.discount > 0 && (
                  <span className="text-content3 relative mr-2 inline-block text-lg">
                    <NumberFlow value={pricing.onDemandPrice} prefix="$" />
                    <span className="pointer-events-none absolute left-0 right-0 top-1/2 h-[1px] bg-current" />
                  </span>
                )}
                <span className="text-accent typography-2xl text-2xl font-bold">
                  <NumberFlow value={pricing.price} prefix="$" />
                </span>
              </div>
              {pricing.discount > 0 && (
                <p className="text-accent-green text-sm font-medium">
                  Save <NumberFlow value={pricing.discount} suffix="%" /> with 1-year commitment
                </p>
              )}
            </div>
          </div>
          <div className="text-md mt-8 flex items-center justify-between gap-4 font-bold">
            <span className="text-content3 flex items-center gap-1">
              <Cpu className="text-accent h-5 w-5" />
              <NumberFlow value={pricingInfo.instanceCPU} />{' '}
              <span className="mb-0.5 self-end text-sm font-medium">CPU</span>
            </span>
            <span className="text-content3 flex items-center gap-1">
              <Database className="text-accent h-5 w-5" />
              <NumberFlow value={pricingInfo.instanceDisk} />{' '}
              <span className="mb-0.5 self-end text-sm font-medium">GB SSD</span>
            </span>
            <span className="text-content3 flex items-center gap-1">
              <MemoryStick className="text-accent h-5 w-5" />
              <NumberFlow value={pricingInfo.instanceRAM} />{' '}
              <span className="mb-0.5 self-end text-sm font-medium">GB RAM</span>
            </span>
            <span className="text-content3 flex items-center gap-1">
              <Zap className="text-accent h-5 w-5" />
              <NumberFlow value={pricingInfo.pageCacheGB} />{' '}
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
          {cta}
        </div>
      </div>
    </div>
  )
}
