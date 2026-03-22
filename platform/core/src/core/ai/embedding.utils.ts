import { createHash } from 'crypto'

/**
 * Produces a canonical text representation for a single string property value.
 * Format: `<propName>: <value>` — used as the text passed to the embedding model.
 */
export function canonicalText(propName: string, value: string): string {
  return `${propName}: ${value}`
}

/**
 * Mean-pools a list of equal-dimension vectors into a single vector.
 * Used for `List<String>` properties (Option B): embed each item then average.
 *
 * @throws if `vectors` is empty or vectors have different lengths
 */
export function meanPool(vectors: number[][]): number[] {
  if (vectors.length === 0) {
    throw new Error('meanPool: vectors array must not be empty')
  }
  const dims = vectors[0].length
  const sum = new Array<number>(dims).fill(0)
  for (const vec of vectors) {
    for (let i = 0; i < dims; i++) {
      sum[i] += vec[i]
    }
  }
  return sum.map((v) => v / vectors.length)
}

/**
 * Builds a stable hash key for a (propName, value, modelKey, dimensions) tuple.
 * For `List<String>` values the list is sorted before hashing so order doesn't matter.
 */
export function buildValHash(
  propName: string,
  value: string | string[],
  modelKey: string,
  dimensions: number
): string {
  const normalised = Array.isArray(value) ? [...value].sort().join('\x00') : value
  return createHash('sha256')
    .update(`${propName}\x00${normalised}\x00${modelKey}\x00${dimensions}`)
    .digest('hex')
}
