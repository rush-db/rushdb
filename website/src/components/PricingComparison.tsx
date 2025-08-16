import { Check, X } from 'lucide-react'
import { ComponentPropsWithoutRef } from 'react'
import cn from 'classnames'

type Feature = {
  name: string
  free: boolean | string
  start: boolean | string
  pro: boolean | string
  enterprise: boolean | string
}

const features: Feature[] = [
  {
    name: 'Projects',
    free: '2',
    start: 'Unlimited',
    pro: 'Unlimited',
    enterprise: 'Unlimited'
  },
  {
    name: 'Records',
    free: '10,000',
    start: 'Up to 200,000',
    pro: 'Up to 1B+',
    enterprise: 'Unlimited'
  },
  {
    name: 'Team Members',
    free: 'N/A',
    start: '3',
    pro: '10',
    enterprise: 'Unlimited'
  },
  {
    name: 'Additional Seat Cost',
    free: 'N/A',
    start: '$10/month',
    pro: '$25/month',
    enterprise: 'Custom'
  },
  {
    name: 'SDKs',
    free: true,
    start: true,
    pro: true,
    enterprise: true
  },
  {
    name: 'REST API',
    free: true,
    start: true,
    pro: true,
    enterprise: true
  },
  {
    name: 'Smart CDN',
    free: true,
    start: true,
    pro: true,
    enterprise: true
  },
  {
    name: 'Dashboard Access',
    free: true,
    start: true,
    pro: true,
    enterprise: true
  },
  {
    name: 'Bring Your Own Neo4j',
    free: false,
    start: true,
    pro: true,
    enterprise: true
  },
  {
    name: 'Cypher Query Preview',
    free: false,
    start: true,
    pro: true,
    enterprise: true
  },
  {
    name: 'Custom Queries',
    free: false,
    start: false,
    pro: true,
    enterprise: true
  },
  {
    name: 'Backups',
    free: false,
    start: false,
    pro: '14-day (Coming Soon)',
    enterprise: 'Custom'
  },
  {
    name: 'Support',
    free: 'Community',
    start: 'Email',
    pro: 'Priority',
    enterprise: 'Dedicated'
  },
  {
    name: 'Dedicated Instances',
    free: false,
    start: false,
    pro: true,
    enterprise: true
  },
  {
    name: 'SSO',
    free: false,
    start: false,
    pro: 'Coming Soon',
    enterprise: true
  },
  {
    name: 'SLA Guarantees',
    free: false,
    start: false,
    pro: false,
    enterprise: true
  },
  {
    name: 'Custom Deployment',
    free: false,
    start: false,
    pro: false,
    enterprise: true
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
              <th className="border-stroke text-content border-b px-6 py-4 text-center font-semibold">
                Start
              </th>
              <th className="border-stroke text-content border-b px-6 py-4 text-center font-semibold">Pro</th>
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
                  <FeatureCell value={feature.start} />
                </td>
                <td className="border-stroke text-content border-b px-6 py-4 text-center">
                  <FeatureCell value={feature.pro} />
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
