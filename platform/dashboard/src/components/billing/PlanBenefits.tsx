import { Check } from 'lucide-react'
import React from 'react'

import type { PlanBenefit } from '~/features/billing/types'

// Fallback used only when the billing service hasn't supplied benefits (offline /
// pre-fetch). Keep in sync with the canonical source in the billing service
// (internal app/api/prices/route.ts → PLAN_BENEFITS).
const benefitsMap: Record<string, PlanBenefit[]> = {
  free: [
    { title: '100K KU / month', description: 'For evaluation and early prototypes' },
    { title: '2 projects' },
    { title: 'Full REST API and SDKs' },
    { title: 'Self-hosted & BYOC' },
    { title: 'Vector & AI search' },
    { title: 'Community support' }
  ],
  start: [
    { title: '250K KU / month', description: 'For solo devs shipping to production' },
    { title: '2 projects' },
    { title: '1 team member' },
    { title: 'Self-hosted & BYOC' },
    { title: 'Community support' }
  ],
  pro: [
    { title: '1M KU / month', description: 'Included before overage' },
    { title: 'Overage billing', description: 'Beyond 1M KU at $10 / M KU' },
    { title: '10 projects' },
    { title: '3 team members', description: 'Add more at $10 / seat' },
    { title: 'Self-hosted & BYOC' }
  ],
  scale: [
    { title: 'Unlimited KU', description: 'Usage-based at $8 / M KU' },
    { title: '100 projects' },
    { title: '10 team members', description: 'Add more at $25 / seat' },
    { title: 'Self-hosted & BYOC' },
    { title: 'SLA guarantee' },
    { title: 'Priority support' }
  ],
  enterprise: [
    { title: 'Platform license', description: 'Unlimited KU, flat-fee pricing' },
    { title: 'Dedicated BYOC deployment' },
    { title: 'Embedded / OEM use' },
    { title: 'Dedicated support' }
  ],
  custom: [
    {
      title: 'Custom commercial terms',
      description: 'Flexible packaging for your deployment and team model'
    },
    { title: 'BYOC or on-premise options', description: 'Run RushDB in your environment when required' },
    { title: 'Solution design support', description: 'Work with us on migration, sizing, and rollout' },
    {
      title: 'Dedicated support path',
      description: 'Prioritized help for production teams and regulated workloads'
    }
  ]
}

export function PlanBenefits({ id, benefits }: { id: string; benefits?: PlanBenefit[] }) {
  // Prefer benefits served by the billing service; fall back to the canonical copy.
  const items = benefits?.length ? benefits : (benefitsMap[id] ?? [])

  return (
    <ul className="flex flex-col gap-1">
      {items.map((benefit) => (
        <li className="flex flex-col" key={benefit.title}>
          <div className="mb-2 flex items-center gap-2">
            <Check className="h-4 w-4" />{' '}
            <div>
              <p>{benefit.title}</p>
              {benefit.description ?
                <p className="text-content3">{benefit.description}</p>
              : null}
            </div>
          </div>
        </li>
      ))}
    </ul>
  )
}
