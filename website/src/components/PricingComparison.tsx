import { Check, X } from 'lucide-react'
import { ComponentPropsWithoutRef } from 'react'
import cn from 'classnames'

type Feature = {
  name: string
  free: boolean | string
  pro: boolean | string
  scale: boolean | string
  enterprise: boolean | string
}

const features: Feature[] = [
  {
    name: 'Knowledge Units / month',
    free: '100K KU',
    pro: '10M KU',
    scale: 'Unlimited',
    enterprise: 'Unlimited'
  },
  {
    name: 'Overage Billing',
    free: false,
    pro: 'Per-KU rate',
    scale: 'Usage-based',
    enterprise: 'Custom'
  },
  {
    name: 'Projects',
    free: '2',
    pro: 'Unlimited',
    scale: 'Unlimited',
    enterprise: 'Unlimited'
  },
  {
    name: 'Team Members',
    free: '1',
    pro: '3',
    scale: 'Unlimited',
    enterprise: 'Unlimited'
  },
  {
    name: 'Additional Seat Cost',
    free: 'N/A',
    pro: '$10/month',
    scale: 'Included',
    enterprise: 'Custom'
  },
  {
    name: 'REST API & SDKs',
    free: true,
    pro: true,
    scale: true,
    enterprise: true
  },
  {
    name: 'Dashboard Access',
    free: true,
    pro: true,
    scale: true,
    enterprise: true
  },
  {
    name: 'Vector & AI Search',
    free: true,
    pro: true,
    scale: true,
    enterprise: true
  },
  {
    name: 'Custom Queries',
    free: false,
    pro: true,
    scale: true,
    enterprise: true
  },
  {
    name: 'Self-Hosted Support',
    free: false,
    pro: true,
    scale: true,
    enterprise: true
  },
  {
    name: 'SLA Guarantee',
    free: false,
    pro: false,
    scale: true,
    enterprise: true
  },
  {
    name: 'Dedicated Instances',
    free: false,
    pro: false,
    scale: true,
    enterprise: true
  },
  {
    name: 'SSO',
    free: false,
    pro: false,
    scale: 'Coming Soon',
    enterprise: true
  },
  {
    name: 'Bring Your Own Cloud (BYOC)',
    free: true,
    pro: true,
    scale: true,
    enterprise: true
  },
  {
    name: 'Embedded / OEM Use',
    free: false,
    pro: false,
    scale: false,
    enterprise: true
  },
  {
    name: 'Support',
    free: 'Community',
    pro: 'Priority',
    scale: 'Priority',
    enterprise: 'Dedicated'
  }
]

const FeatureCell = ({ value }: { value: boolean | string }) => {
  if (typeof value === 'boolean') {
    return value ?
        <Check className="text-accent mx-auto h-5 w-5" />
      : <X className="text-content3 mx-auto h-5 w-5" />
  }

  if (typeof value === 'string' && value.includes('Coming Soon')) {
    return (
      <span className="text-content flex flex-col items-center font-medium">
        <span className="text-sm">{value.replace(' (Coming Soon)', '')}</span>
        <span className="text-accent text-xs italic">Coming Soon</span>
      </span>
    )
  }

  return <span className="text-content font-medium">{value}</span>
}

export const PricingComparison = ({ className, ...props }: ComponentPropsWithoutRef<'div'>) => {
  return (
    <div className={cn('w-full', className)} {...props}>
      <div className="mb-8 text-center">
        <h3 className="mb-4 text-2xl font-bold">Feature Comparison</h3>
        <p className="text-content3">Compare plans to find what works best for your project</p>
      </div>

      <div className="overflow-x-auto rounded-sm">
        <table className="border-stroke bg-fill w-full border-collapse rounded-lg border shadow-sm">
          <thead>
            <tr className="bg-secondary">
              <th className="border-stroke text-content border-b px-6 py-4 text-left font-semibold">
                Features
              </th>
              <th className="border-stroke text-content border-b px-6 py-4 text-center font-semibold">
                Free
              </th>
              <th className="border-stroke text-content border-b px-6 py-4 text-center font-semibold">Pro</th>
              <th className="border-stroke text-content border-b px-6 py-4 text-center font-semibold">
                Scale
              </th>
              <th className="border-stroke text-content border-b px-6 py-4 text-center font-semibold">
                Enterprise
              </th>
            </tr>
          </thead>
          <tbody>
            {features.map((feature, index) => (
              <tr
                key={feature.name}
                className={`${index % 2 === 0 ? 'bg-fill' : 'bg-secondary'} hover:bg-fill2 transition-colors`}
              >
                <td className="border-stroke text-content border-b px-6 py-4 font-medium">{feature.name}</td>
                <td className="border-stroke text-content border-b px-6 py-4 text-center">
                  <FeatureCell value={feature.free} />
                </td>
                <td className="border-stroke text-content border-b px-6 py-4 text-center">
                  <FeatureCell value={feature.pro} />
                </td>
                <td className="border-stroke text-content border-b px-6 py-4 text-center">
                  <FeatureCell value={feature.scale} />
                </td>
                <td className="border-stroke text-content border-b px-6 py-4 text-center">
                  <FeatureCell value={feature.enterprise} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}
