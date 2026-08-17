/**
 * Pure helpers for per-stream sequence enforcement in the `_internal/synx`
 * destination. Kept side-effect free so the ordering rules are unit-testable
 * without a database.
 */

/** Checkpoint stored for one `(bindingId, streamId)` in `connector_offsets`. */
export interface SynxOffsetCheckpoint {
  sequence: number
  updatedAt: string
}

/** Prefix that scopes a synx stream checkpoint partition in `connector_offsets`. */
export const SYNX_OFFSET_PARTITION_PREFIX = 'synx:'

export function synxPartition(bindingId: string, streamId: string): string {
  return `${SYNX_OFFSET_PARTITION_PREFIX}${bindingId}:${streamId}`
}

/** Decode the JSON checkpoint `position` column. Returns null for missing/bad values. */
export function parseOffsetPosition(raw: string | null | undefined): SynxOffsetCheckpoint | null {
  if (!raw) {
    return null
  }
  try {
    const parsed = JSON.parse(raw)
    if (typeof parsed?.sequence !== 'number') {
      return null
    }
    return {
      sequence: parsed.sequence,
      updatedAt: typeof parsed.updatedAt === 'string' ? parsed.updatedAt : ''
    }
  } catch {
    return null
  }
}

export function stringifyOffsetCheckpoint(checkpoint: SynxOffsetCheckpoint): string {
  return JSON.stringify(checkpoint)
}

/**
 * Decide what to do with an incoming batch sequence given the last committed
 * checkpoint (`null` when none exists for the stream).
 */
export function decideSequence(
  current: SynxOffsetCheckpoint | null,
  incoming: number
): 'accept' | 'duplicate' | 'gap' {
  const expected = current === null ? 0 : current.sequence + 1
  if (incoming === expected) {
    return 'accept'
  }
  if (incoming < expected) {
    return 'duplicate'
  }
  return 'gap'
}
