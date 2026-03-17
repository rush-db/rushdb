import { Check } from 'lucide-react'
import React from 'react'

const benefitsMap: Record<string, Array<{ description?: string; title: string }>> = {
  free: [
    { title: '100K KU / month', description: '~3,000 records with 10 fields each' },
    { title: '2 projects', description: 'No time limits, no feature restrictions' },
    { title: 'Full REST API and SDKs', description: 'Complete access to all query capabilities' },
    { title: 'Vector & AI search', description: 'Native support for embeddings and similarity' },
    { title: 'Community support', description: 'Get help from our developer community' }
  ],
  start: [
    { title: '100K KU / month', description: '~3,000 records with 10 fields each' },
    { title: '2 projects', description: 'No time limits, no feature restrictions' },
    { title: 'Full REST API and SDKs', description: 'Complete access to all query capabilities' },
    { title: 'Vector & AI search', description: 'Native support for embeddings and similarity' },
    { title: 'Community support', description: 'Get help from our developer community' }
  ],
  pro: [
    { title: '10M KU / month', description: '~300,000 records with 10 fields each' },
    { title: 'Overage billing', description: 'Continue beyond 10M KU at a per-KU rate' },
    { title: 'Unlimited projects', description: 'No per-project limits' },
    { title: '3 team members', description: 'then $10 per member' },
    { title: 'Self-hosted support', description: 'Deploy on your own infrastructure' }
  ],
  scale: [
    { title: 'Unlimited KU', description: 'Usage-based billing — pay only for what you consume' },
    { title: 'Unlimited team members', description: 'No per-seat limit' },
    { title: 'SLA guarantee', description: 'Uptime and performance guarantees' },
    { title: 'Daily backups stored for 14 days', description: 'Coming soon' },
    { title: 'Priority support', description: 'Fast-track access to expert help' }
  ],
  enterprise: [
    { title: 'Platform license', description: 'Unlimited KU, flat-fee pricing' },
    { title: 'Bring Your Own Cloud (BYOC)', description: 'Deploy in your private cloud or on-premise' },
    { title: 'Embedded / OEM use', description: 'Build RushDB into your own product' },
    { title: 'Dedicated support', description: 'Phone, chat, and dedicated account manager' }
  ]
}

export function PlanBenefits({ id }: { id: string }) {
  const benefits = benefitsMap[id] ?? []

  return (
    <ul className="flex flex-col gap-1">
      {benefits.map((benefit) => (
        <li className="flex flex-col" key={benefit.title}>
          <div className="mb-2 flex items-center gap-2">
            <Check className="h-4 w-4" />{' '}
            <div>
              <p>{benefit.title}</p>
              <p className="text-content3">{benefit.description}</p>
            </div>
          </div>
        </li>
      ))}
    </ul>
  )
}
