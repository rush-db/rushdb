import { SynxConnectorRegistry } from './synx.connectors'

import type { ConnectorRepository } from '@/dashboard/connector/model/connector.repository'
import type { SynxConnectorV1 } from '@rushdb/synx-contract'

const postgres: SynxConnectorV1 = {
  id: 'postgres',
  type: 'database',
  name: 'PostgreSQL',
  version: '0.1.0',
  schemaVersion: '1',
  capabilities: {
    batchModes: ['snapshot', 'incremental', 'replay'],
    deletionModes: ['ignore', 'soft_delete', 'hard_delete'],
    relationEvidence: true
  },
  entitlement: 'free',
  fields: [{ key: 'database', label: 'Database', type: 'string', required: true }]
}

const hubspot: SynxConnectorV1 = {
  id: 'hubspot',
  type: 'saas',
  name: 'HubSpot',
  version: '0.1.0',
  schemaVersion: '1',
  capabilities: {
    batchModes: ['snapshot', 'incremental', 'replay'],
    deletionModes: ['ignore', 'soft_delete', 'hard_delete'],
    relationEvidence: true,
    oauth: true,
    webhooks: true
  },
  entitlement: 'top_tier',
  fields: []
}

function repo(overrides: Record<string, unknown> = {}): ConnectorRepository {
  const base = {
    upsertConnectorDefinition: jest.fn(async () => undefined),
    listConnectorDefinitions: jest.fn(async () => [
      { id: 'postgres', descriptor: JSON.stringify(postgres), version: '1', updatedAt: '' },
      { id: 'hubspot', descriptor: JSON.stringify(hubspot), version: '1', updatedAt: '' }
    ]),
    findConnectorDefinition: jest.fn(async (id: string) =>
      id === 'postgres' ?
        { id: 'postgres', descriptor: JSON.stringify(postgres), version: '1', updatedAt: '' }
      : undefined
    )
  }
  return { ...base, ...overrides } as unknown as ConnectorRepository
}

describe('SynxConnectorRegistry', () => {
  it('serves the catalog from registered definitions (not hardcoded)', async () => {
    const registry = new SynxConnectorRegistry(repo())
    const { connectors } = await registry.list()
    expect(connectors.length).toBe(2)
    const pg = connectors.find((c) => c.id === 'postgres')
    expect(pg?.capabilities.relationEvidence).toBe(true)
  })

  it('partitions by plan using registered entitlement tiers', async () => {
    const registry = new SynxConnectorRegistry(repo())
    const free = await registry.listForPlan('free')
    expect(free.connectors.map((c) => c.id)).toEqual(['postgres'])
    expect(free.unavailable.map((u) => u.id)).toContain('hubspot')

    const top = await registry.listForPlan('enterprise')
    expect(top.connectors.length).toBe(2)
  })

  it('register upserts descriptors and reflects them in the catalog', async () => {
    const upsert = jest.fn(async () => undefined)
    const list = jest.fn(async () => [
      {
        id: 'stripe',
        descriptor: JSON.stringify({ ...hubspot, id: 'stripe', name: 'Stripe' }),
        version: '1',
        updatedAt: ''
      }
    ])
    const registry = new SynxConnectorRegistry(
      repo({ upsertConnectorDefinition: upsert, listConnectorDefinitions: list })
    )
    const catalog = await registry.register([
      { ...hubspot, id: 'stripe', name: 'Stripe', entitlement: 'paid' }
    ])
    expect(upsert).toHaveBeenCalledWith(expect.objectContaining({ id: 'stripe' }))
    expect(catalog.connectors.map((c) => c.id)).toContain('stripe')
  })
})
