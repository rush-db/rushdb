import { Injectable } from '@nestjs/common'
import { SynxConnectorCatalogV1, SynxConnectorV1, catalogForPlan } from '@rushdb/synx-contract'

import { ConnectorRepository } from '@/dashboard/connector/model/connector.repository'

/**
 * Registry of connector descriptors served to the dashboard.
 *
 * Core never hardcodes the connector union. Descriptors are *runtime-registered
 * rows* (`connector_definitions`) reported by synx workers; adding a connector
 * is a worker registering its spec — no rushdb redeploy/restart.
 *
 * Descriptors are *shape only* — labels, field schemas, capabilities, and the
 * entitlement tier — never configuration values or secrets.
 */
@Injectable()
export class SynxConnectorRegistry {
  constructor(private readonly connectorRepository: ConnectorRepository) {}

  /**
   * Full catalog (all registered connectors). Used by the internal sync
   * endpoint and self-hosted mode where plan gating is disabled.
   */
  async list(): Promise<SynxConnectorCatalogV1> {
    const connectors = await this.loadAll()
    return { connectors, unavailable: [] }
  }

  /**
   * Catalog partitioned for a workspace plan. Synx owns the provider union and
   * each connector's minimum tier (carried in the registered descriptor); Core
   * only forwards the workspace plan id.
   */
  async listForPlan(planId?: string): Promise<SynxConnectorCatalogV1> {
    const connectors = await this.loadAll()
    return catalogForPlan(connectors, planId)
  }

  /**
   * Register (upsert) one or more connector descriptors reported by a worker.
   * This is the only way the catalog grows; Core stores opaque descriptor rows.
   */
  async register(descriptors: SynxConnectorV1[]): Promise<SynxConnectorCatalogV1> {
    const now = new Date().toISOString()
    for (const descriptor of descriptors) {
      if (!descriptor?.id || !descriptor?.name) {
        continue
      }
      await this.connectorRepository.upsertConnectorDefinition({
        id: descriptor.id,
        descriptor: JSON.stringify(descriptor),
        version: descriptor.schemaVersion ?? '1',
        registeredBy: 'synx-worker',
        updatedAt: now
      })
    }
    return this.list()
  }

  async find(id: string): Promise<SynxConnectorV1 | undefined> {
    const row = await this.connectorRepository.findConnectorDefinition(id)
    return row ? this.parseDescriptor(row.descriptor) : undefined
  }

  /** True when a spec with this id is registered (database or spec connector). */
  async isRegistered(id: string): Promise<boolean> {
    const row = await this.connectorRepository.findConnectorDefinition(id)
    return Boolean(row)
  }

  private async loadAll(): Promise<SynxConnectorV1[]> {
    const rows = await this.connectorRepository.listConnectorDefinitions()
    return rows
      .map((row) => this.parseDescriptor(row.descriptor))
      .filter((d): d is SynxConnectorV1 => d !== undefined)
  }

  private parseDescriptor(raw: string): SynxConnectorV1 | undefined {
    try {
      const parsed = JSON.parse(raw) as SynxConnectorV1
      return parsed?.id && parsed?.name ? parsed : undefined
    } catch {
      return undefined
    }
  }
}
