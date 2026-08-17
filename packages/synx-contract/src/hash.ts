/**
 * Deterministic JSON canonicalization + hashing shared by TypeScript (Core) and
 * Rust (Synx). Both implementations must produce byte-identical results for
 * the same input so source identities and schema hashes agree across repos.
 *
 * Contract: inputs are JSON-serializable values with integer numbers, strings,
 * booleans, null, arrays, and objects. Non-integer floats are not guaranteed
 * byte-identical across implementations and are not part of the canonical hash
 * contract.
 */

import { createHash } from 'node:crypto'

/** Deep-copy `value` with object keys recursively sorted (arrays keep order). */
export function sortKeys(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sortKeys)
  }
  if (value !== null && typeof value === 'object') {
    const record = value as Record<string, unknown>
    const sorted: Record<string, unknown> = {}
    for (const key of Object.keys(record).sort()) {
      sorted[key] = sortKeys(record[key])
    }
    return sorted
  }
  return value
}

/** Compact JSON with sorted object keys and no insignificant whitespace. */
export function canonicalJsonString(value: unknown): string {
  return JSON.stringify(sortKeys(value))
}

/** SHA-256 (hex) of the canonical JSON representation of `value`. */
export function canonicalJsonHash(value: unknown): string {
  return createHash('sha256').update(canonicalJsonString(value)).digest('hex')
}

/**
 * Deterministic source identity hash. `parts` are the source-identity
 * components in a stable order (e.g. `[streamName, ...pkValues]`). Core stores
 * this hash as the internal record identity; Synx uses it to correlate.
 */
export function hashSourceIdentity(parts: unknown[]): string {
  return canonicalJsonHash(parts)
}

/**
 * SHA-256 of the canonical serialization of `{ [filename]: schema }` for a
 * schema set. Sorting the filename keys first keeps the hash independent of
 * declaration order and identical across TypeScript and Rust.
 */
export function computeSchemaHash(schemas: Record<string, unknown>): string {
  return canonicalJsonHash(schemas)
}
