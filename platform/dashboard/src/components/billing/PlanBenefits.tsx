import { Check } from 'lucide-react'
import React from 'react'

const benefitsMap: Record<string, Array<{ description?: string; title: string }>> = {
  start: [
    { title: 'Up to 10,000 records', description: 'Perfect for prototypes and small apps' },
    { title: '2 projects', description: 'No time limits, no feature restrictions' },
    { title: 'Full REST API and SDKs', description: 'Complete access to all query capabilities' },
    { title: 'Smart CDN', description: 'Global API endpoints for faster access' },
    { title: 'Community support', description: 'Get help from our developer community' }
  ],
  pro: [
    {
      title: 'Up to 200,000 records',
      description: 'On shared infrastructure. Unlimited with your own Neo4j instance'
    },
    { title: 'Unlimited projects', description: 'Record limit applies collectively across all projects' },
    { title: 'Bring your Neo4j', description: 'Connect Neo4j Aura or own instance' },
    { title: '3 team members', description: 'then $10 per member' }
  ],
  team: [
    { title: 'Up to 1B+ records', description: 'Instant scale up with a single click—grow as needed' },
    { title: 'Dedicated instances', description: 'Guaranteed performance, security, and throughput' },
    { title: '10 team members', description: 'then $25 per member' },
    { title: 'Daily backups stored for 14 days', description: 'Coming soon' },
    { title: 'SSO', description: 'Enterprise-grade authentication (coming soon)' },
    { title: 'Priority support', description: 'Fast-track access to expert help' }
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
