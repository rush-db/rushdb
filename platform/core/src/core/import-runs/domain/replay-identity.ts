import { createHash } from 'node:crypto'

const IMPORT_REPLAY_NAMESPACE = '6f1d0c9e-3a2b-4c5d-8e7f-90a1b2c3d4e5'

/**
 * Deterministic record identity for async import replay.
 *
 * Derived from project + file + source generation + unit ordinal + stable nested
 * path with sibling occurrence. Never hashes raw property values. Stable across
 * retries of the same source generation, distinct across runs/sources.
 */
export function deriveDeterministicRecordId(input: {
  projectId: string
  fileId: string
  sourceGeneration: number
  unitOrdinal: number
  path: string
  siblingOccurrence: number
}): string {
  const payload = [
    IMPORT_REPLAY_NAMESPACE,
    input.projectId,
    input.fileId,
    String(input.sourceGeneration),
    String(input.unitOrdinal),
    input.path,
    String(input.siblingOccurrence)
  ].join('\n')

  const digest = createHash('sha256').update(payload).digest('hex').slice(0, 32)

  // Format as UUIDv8-shaped (version 8, RFC variant) deterministic identifier.
  return [
    digest.slice(0, 8),
    digest.slice(8, 12),
    `8${digest.slice(13, 16)}`,
    ((parseInt(digest[16], 16) & 0x3) | 0x8).toString(16) + digest.slice(17, 20),
    digest.slice(20, 32)
  ].join('-')
}

export function hashIdempotencyKey(key: string): string {
  return createHash('sha256').update(key).digest('hex')
}

export function deriveBatchId(fileId: string, sourceGeneration: number, batchOrdinal: number): string {
  return createHash('sha256')
    .update([fileId, sourceGeneration, batchOrdinal].join('\n'))
    .digest('hex')
    .slice(0, 24)
}
