import { Check, X } from 'lucide-react'

import { cn } from '~/lib/utils.ts'
import { Skeleton } from '~/elements/Skeleton.tsx'
import { usePricingDataQuery } from '~/features/billing/hooks/useBillingHooks'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import type { ComparisonFeature } from '~/features/billing/types'

// Canonical fallback (source of truth: billing service /api/prices → comparison).
// Rendered when the billing service hasn't provided a live matrix, so the table
// stays correct when pricing is unavailable.
const FALLBACK_COMPARISON: ComparisonFeature[] = [
  {
    name: 'Knowledge Units / month',
    free: '100K KU',
    start: '250K KU',
    pro: '1M KU',
    scale: 'Unlimited',
    enterprise: 'Unlimited'
  },
  {
    name: 'Overage Billing',
    free: false,
    start: false,
    pro: '$10 / M KU',
    scale: '$8 / M KU',
    enterprise: 'Custom'
  },
  { name: 'Projects', free: '2', start: '2', pro: '10', scale: '100', enterprise: 'Unlimited' },
  { name: 'Team Members', free: '1', start: '1', pro: '3', scale: '10', enterprise: 'Unlimited' },
  {
    name: 'Additional Seat Cost',
    free: 'N/A',
    start: 'N/A',
    pro: '$10 / seat',
    scale: '$25 / seat',
    enterprise: 'Custom'
  },
  { name: 'REST API & SDKs', free: true, start: true, pro: true, scale: true, enterprise: true },
  { name: 'Dashboard Access', free: true, start: true, pro: true, scale: true, enterprise: true },
  { name: 'Vector & AI Search', free: true, start: true, pro: true, scale: true, enterprise: true },
  { name: 'Custom Queries', free: false, start: true, pro: true, scale: true, enterprise: true },
  { name: 'Self-Hosted Support', free: false, start: true, pro: true, scale: true, enterprise: true },
  { name: 'SLA Guarantee', free: false, start: false, pro: false, scale: true, enterprise: true },
  { name: 'Dedicated Instances', free: false, start: false, pro: false, scale: true, enterprise: true },
  { name: 'SSO', free: false, start: false, pro: false, scale: 'Coming Soon', enterprise: true },
  {
    name: 'Bring Your Own Cloud (BYOC)',
    free: true,
    start: true,
    pro: true,
    scale: true,
    enterprise: true
  },
  { name: 'Embedded / OEM Use', free: false, start: false, pro: false, scale: false, enterprise: true },
  {
    name: 'Support',
    free: 'Community',
    start: 'Community',
    pro: 'Priority',
    scale: 'Priority',
    enterprise: 'Dedicated'
  }
]

const PLAN_KEYS = ['free', 'start', 'pro', 'scale', 'enterprise'] as const
const PLAN_LABELS = ['Free', 'Start', 'Pro', 'Scale', 'Enterprise']

function FeatureCell({ value }: { value: boolean | string }) {
  if (typeof value === 'boolean') {
    return value ?
        <Check className="text-accent mx-auto h-4 w-4" />
      : <X className="text-content3 mx-auto h-4 w-4" />
  }

  if (value === 'Coming Soon') {
    return <span className="text-accent italic">Coming soon</span>
  }

  return <span className="text-content">{value}</span>
}

function ComparisonHeading() {
  return (
    <div className="mb-6 text-center">
      <p className="text-content3 mb-2 font-mono text-xs uppercase tracking-widest">Compare plans</p>
      <p className="text-content font-mono text-xl font-bold">Feature comparison</p>
    </div>
  )
}

function ComparisonSkeleton() {
  return (
    <div className="w-full">
      <ComparisonHeading />
      <Skeleton enabled className="h-96 w-full rounded-2xl" />
    </div>
  )
}

/**
 * Feature-by-feature plan comparison table, mirroring the website's /pricing table.
 * Uses live data from the billing service when available, otherwise the canonical
 * fallback. Shows a skeleton while pricing is still loading.
 */
export function PricingComparison({ className }: { className?: string }) {
  const { data: settings, isPending: settingsPending } = usePlatformSettings()
  const { data: billingData, isLoading: pricingLoading } = usePricingDataQuery()

  // Match the Plans grid: skeleton until tier is known and, for hosted workspaces,
  // until pricing has loaded.
  const showSkeleton = settingsPending || (settings?.selfHosted === false && pricingLoading)

  if (showSkeleton) {
    return <ComparisonSkeleton />
  }

  const features = billingData?.comparison ?? FALLBACK_COMPARISON

  return (
    <div className={cn('w-full', className)}>
      <ComparisonHeading />

      <div className="overflow-x-auto rounded-2xl border">
        <table className="w-full border-collapse font-mono text-sm">
          <thead>
            <tr className="bg-secondary">
              <th className="text-content border-b px-5 py-4 text-left font-semibold">Features</th>
              {PLAN_LABELS.map((label) => (
                <th className="text-content border-b px-5 py-4 text-center font-semibold" key={label}>
                  {label}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {features.map((feature, index) => (
              <tr className={index % 2 ? 'bg-secondary' : undefined} key={feature.name}>
                <td className="text-content border-b px-5 py-4 text-left">{feature.name}</td>
                {PLAN_KEYS.map((key) => (
                  <td className="text-content2 border-b px-5 py-4 text-center" key={key}>
                    <FeatureCell value={feature[key]} />
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}
