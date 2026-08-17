/**
 * Service capability negotiation, v1.
 *
 * Returned by `GET /api/v1/_internal/synx/capabilities` (service identity only).
 * Used during health/claim so Synx and Core agree on a supported contract
 * version and limits before any batch is emitted.
 */

export interface SynxContractLimitsV1 {
  /** Hard v1 limit for one envelope (configurable by measured Cloud performance). */
  maxOperationsPerBatch: number
  /** Hard v1 limit for one envelope body (configurable by measured Cloud performance). */
  maxBytesPerBatch: number
}

export interface SynxCapabilitiesV1 {
  contractVersion: '1'
  /** SHA-256 of the canonicalized schema set this service validates against. */
  schemaHash: string
  serviceName: string
  limits: SynxContractLimitsV1
  supportedDeletionModes: string[]
  supportedBatchModes: string[]
}
