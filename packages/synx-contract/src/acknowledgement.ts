/**
 * Destination acknowledgement, v1.
 *
 * Returned by the Cloud Core destination for every submitted envelope. Synx
 * advances the source checkpoint only when `status === "committed"` and
 * `checkpointAccepted === true`.
 */

export const SYNX_ACK_STATUSES = ['committed', 'rejected', 'duplicate'] as const
export type SynxAckStatusV1 = (typeof SYNX_ACK_STATUSES)[number]

export interface SynxAckErrorV1 {
  /** Index into the envelope `operations` array when the error is per-operation. */
  operationIndex?: number
  /** Hash of the offending source identity (never the raw identity/payload). */
  sourceIdHash?: string
  /** Stable error code (see `errors.ts`). */
  code: string
  /** Human-readable, redacted summary. */
  message: string
  retryable: boolean
}

export interface SynxAcknowledgementV1 {
  batchId: string
  status: SynxAckStatusV1
  accepted: number
  rejected: number
  duplicate: boolean
  /** True only when Core durably accepted the batch's checkpoint position. */
  checkpointAccepted: boolean
  /** Contract schema version applied by Core (`1`). */
  schemaVersion: string
  committedAt?: string
  /** Present when Core rejected the batch for ordering reasons (next expected value). */
  nextExpectedSequence?: number
  errors: SynxAckErrorV1[]
}
