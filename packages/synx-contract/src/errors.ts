/**
 * Stable error codes for the private Cloud destination contract.
 *
 * Codes are additive; unknown codes must be treated as non-retryable by
 * consumers. `retryable` on an error detail overrides the code-level default
 * for a specific operation.
 */

export const SYNX_ERROR_CODES = [
  'INVALID_CONTRACT_VERSION',
  'INVALID_BINDING',
  'BINDING_INACTIVE',
  'ENTITLEMENT_REJECTED',
  'SEQUENCE_GAP',
  'SEQUENCE_STALE',
  'MALFORMED_OPERATION',
  'MAPPING_VERSION_STALE',
  'SCHEMA_VERSION_STALE',
  'LIMIT_EXCEEDED',
  'RATE_LIMITED',
  'UPSTREAM_UNAVAILABLE',
  'INTERNAL'
] as const

export type SynxErrorCode = (typeof SYNX_ERROR_CODES)[number]

/** Whether a code is safe to retry by default. */
export function isRetryableCode(code: string): boolean {
  return code === 'RATE_LIMITED' || code === 'UPSTREAM_UNAVAILABLE' || code === 'INTERNAL'
}

/**
 * HTTP error body used by `_internal/synx` routes. The source binding / batch
 * details are carried by the caller's credentials and the request path, never
 * in the body.
 */
export interface SynxErrorResponseV1 {
  code: SynxErrorCode | string
  message: string
  retryable: boolean
  details?: Record<string, unknown>
}
