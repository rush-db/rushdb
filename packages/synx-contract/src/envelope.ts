/**
 * Private Cloud destination envelope, v1.
 *
 * One envelope targets exactly one Cloud source binding and one stream. It is
 * the single batch unit exchanged between RushDB Synx workers and the Cloud
 * Core destination (`POST /api/v1/_internal/synx/batches`).
 *
 * Version policy: additive optional fields stay compatible within v1; enum
 * expansion is breaking unless both services negotiate the new version first.
 * Services negotiate supported versions during health/claim and never silently
 * downgrade.
 */

/** Batch submission modes. `replay` is emitted by recovery/resync paths. */
export const SYNX_BATCH_MODES = ['snapshot', 'incremental', 'replay'] as const
export type SynxBatchModeV1 = (typeof SYNX_BATCH_MODES)[number]

/** How source deletions are applied to the destination graph. */
export const SYNX_DELETION_MODES = ['ignore', 'soft_delete', 'hard_delete'] as const
export type SynxDeletionModeV1 = (typeof SYNX_DELETION_MODES)[number]

export interface SynxSourceV1 {
  /** Provisioned Cloud source binding (control-plane issued). */
  bindingId: string
  /** Owning connection. */
  connectionId: string
  /** Connector type, e.g. `postgres`, `mongodb`, `hubspot`. */
  connectorType: string
  /** Connector instance/account qualifier when the type needs one. */
  instance?: string
}

export interface SynxStreamV1 {
  id: string
  name: string
  namespace?: string
  /** Source-side schema version this envelope was produced against. */
  sourceSchemaVersion?: string
}

export interface SynxBatchV1 {
  /** Unique within a binding. */
  id: string
  /**
   * Strictly increasing per `(binding, stream)`. Core accepts exactly the next
   * committed sequence; gaps/stale sequences are rejected with a stable 409.
   * A worker allocates a sequence only while holding a current fenced lease.
   */
  sequence: number
  /**
   * Connector generation the batch belongs to. Core rejects batches carrying a
   * generation older than the connector's current generation (e.g. after a
   * resnapshot), fencing stale workers out of the new snapshot.
   */
  generation?: number
  /** ISO-8601 UTC. */
  emittedAt: string
  /** Opaque source cursor hash for audit/correlation. Cursor content never leaves Synx. */
  cursorHash?: string
  mode: SynxBatchModeV1
}

export interface SynxMappingV1 {
  /** Singular `UPPER_CASE` RushDB label. */
  targetLabel: string
  /** Source fields that identify a record (hint; the durable identity is Core-computed). */
  identityFields: string[]
  deletionMode: SynxDeletionModeV1
  /** Immutable once used; editing a mapping creates a new approved version. */
  mappingVersion: number
}

export interface SynxRelationTargetV1 {
  streamId: string
  sourceId: string
}

/**
 * Connector-declared source relation evidence, e.g. a HubSpot association or a
 * database foreign key. Core owns actual relationship creation and provenance.
 * `type` is a `UPPER_SNAKE_CASE` verb (`PLACED_BY`, `ASSOCIATED_WITH`).
 */
export interface SynxSourceRelationReferenceV1 {
  type: string
  to: SynxRelationTargetV1
}

export interface SynxUpsertOperationV1 {
  type: 'upsert'
  sourceId: string
  sourceCreatedAt?: string
  sourceUpdatedAt?: string
  /** Source record fields, preserved verbatim. Nested objects are allowed. */
  data: Record<string, unknown>
  relations?: SynxSourceRelationReferenceV1[]
}

export interface SynxDeleteOperationV1 {
  type: 'delete'
  sourceId: string
  sourceDeletedAt?: string
}

export type SynxOperationV1 = SynxUpsertOperationV1 | SynxDeleteOperationV1

export interface SynxEnvelopeV1 {
  version: '1'
  source: SynxSourceV1
  stream: SynxStreamV1
  batch: SynxBatchV1
  mapping: SynxMappingV1
  operations: SynxOperationV1[]
}
