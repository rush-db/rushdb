import { createHash } from 'node:crypto'

/** Measurements of record writes for usage telemetry. */

export interface WriteDimensions {
  /** Count of attached scalar values written. */
  scalarValuesWritten: number
  /** Serialized byte size of the written scalar values. */
  valueBytesWritten: number
}

/** Serialized byte length of a scalar property value; non-scalars weigh zero. */
export function scalarValueBytes(value: unknown): number {
  if (value === null || value === undefined) {
    return 0
  }

  switch (typeof value) {
    case 'string':
      return Buffer.byteLength(value, 'utf8')
    case 'number':
      return 8
    case 'boolean':
      return 1
    default:
      return 0
  }
}

/**
 * Measures a record write: how many scalar values were attached and how many
 * bytes they serialize to. Nested objects/arrays are ignored — RushDB
 * decomposes them into separate linked records, each measured on its own
 * write path.
 */
export function measureWrite(properties: Record<string, unknown>): WriteDimensions {
  let scalarValues = 0
  let bytes = 0

  for (const value of Object.values(properties ?? {})) {
    if (value !== null && value !== undefined && typeof value === 'object') {
      continue
    }
    if (value !== null && value !== undefined) {
      scalarValues += 1
      bytes += scalarValueBytes(value)
    }
  }

  return { scalarValuesWritten: scalarValues, valueBytesWritten: bytes }
}

/** Counts relationships formed by an operation. */
export function measureRelationships(count: number): number {
  return Math.max(0, Math.floor(count))
}

/**
 * Transport idempotency key: stable across delivery retries and identical
 * for duplicate emissions of the same logical operation.
 */
export function buildIdempotencyKey(
  workspaceId: string,
  projectId: string,
  requestId: string,
  operationKey: string
): string {
  return createHash('sha256').update(`${workspaceId}${projectId}${requestId}${operationKey}`).digest('hex')
}
