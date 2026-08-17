/**
 * Connector capability + configuration descriptor, v1.
 *
 * Served by the Synx control plane so Cloud learns about available source
 * connectors without a rushdb redeploy. The dashboard renders "add source"
 * flows generically from the declared `fields` and this descriptor, so adding a
 * new connector to a deployed Synx instance surfaces it in the UI immediately.
 *
 * The catalog exposes *shape* only — labels, field schemas, capabilities —
 * never stored configuration values or secrets.
 */

import type { SynxBatchModeV1, SynxDeletionModeV1 } from './envelope'

export interface SynxConnectorFieldOptionV1 {
  value: string
  label: string
}

/** Form input kinds a connector's config form may use. */
export const SYNX_CONNECTOR_FIELD_TYPES = ['string', 'integer', 'boolean', 'select'] as const
export type SynxConnectorFieldTypeV1 = (typeof SYNX_CONNECTOR_FIELD_TYPES)[number]

export interface SynxConnectorFieldV1 {
  /** Config key, lower-camel stable identifier (durable across schema versions). */
  key: string
  /** Human-readable field label. */
  label: string
  type: SynxConnectorFieldTypeV1
  required: boolean
  description?: string
  placeholder?: string
  /** Masked at rest; the value is never returned by the catalog. */
  secret?: boolean
  /** Required when `type === "select"`. */
  options?: SynxConnectorFieldOptionV1[]
}

export interface SynxConnectorCapabilitiesV1 {
  batchModes: SynxBatchModeV1[]
  deletionModes: SynxDeletionModeV1[]
  /** Whether the connector emits source relation evidence on upserts. */
  relationEvidence: boolean
  /** SaaS connectors authenticate via the Cloud OAuth broker. */
  oauth?: boolean
  /** Provider pushes live events via Cloud webhook ingress + poll reconciliation. */
  webhooks?: boolean
}

/** Provider entitlement tier: which plans may use a connector. */
export const SYNX_CONNECTOR_TIERS = ['free', 'paid', 'top_tier'] as const
export type SynxConnectorTierV1 = (typeof SYNX_CONNECTOR_TIERS)[number]

export interface SynxConnectorV1 {
  /** Stable connector id, e.g. `intercom`, `postgres`. */
  id: string
  /** Connector category, e.g. `crm`, `database`, `saas`. */
  type: string
  /** Display name. */
  name: string
  description?: string
  /** Connector plugin version (semver). */
  version: string
  /**
   * Descriptor schema version. Additive within `1`; bump only for breaking shape
   * changes. Consumers must tolerate connectors declaring a later version.
   */
  schemaVersion: string
  capabilities: SynxConnectorCapabilitiesV1
  /**
   * Minimum plan tier required to use this connector. `free` = every workspace;
   * `paid` = any non-free plan; `top_tier` = highest tier only. Synx owns this
   * decision — Core only forwards the workspace plan.
   */
  entitlement?: SynxConnectorTierV1
  /** Human-readable reason shown when the workspace cannot use the connector. */
  unavailableReason?: string
  /** Configuration form fields (never value-bearing). */
  fields: SynxConnectorFieldV1[]
}

/**
 * Catalog payload returned to the dashboard. `connectors` are the providers the
 * requesting workspace may use; `unavailable` are visible but locked entries
 * (with the reason / required tier) so the UI can prompt an upgrade instead of
 * hiding the provider entirely. The catalog is computed from the requesting
 * workspace's plan by the synx provider registry — Core never hardcodes the
 * connector union.
 */
export interface SynxConnectorCatalogV1 {
  connectors: SynxConnectorV1[]
  unavailable: SynxUnavailableConnectorV1[]
}

/** A provider the workspace cannot use yet, shown only so an upgrade is visible. */
export interface SynxUnavailableConnectorV1 {
  id: string
  name: string
  /** Minimum tier required (e.g. `paid`, `top_tier`). */
  requiredTier: SynxConnectorTierV1
  reason: string
  /** The connector's spec icon, shown grayscaled so locked tiles stay recognizable. */
  icon?: string
}
