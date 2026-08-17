/**
 * Contract version policy and negotiation.
 *
 * Services negotiate supported versions during health/claim. A version is
 * mutually supported only when both sides list it. Negotiation never silently
 * downgrades to a version only one side supports — if no version matches, the
 * caller must fail loudly.
 */

export const CONTRACT_VERSION = '1' as const

export interface SynxNegotiation {
  version: string
  schemaHash: string
}

/**
 * Pick the highest mutually supported version. Returns `null` when the caller
 * and server share no supported version (a hard deployment error).
 */
export function negotiateContract(clientVersions: string[], serverVersions: string[]): string | null {
  const shared = clientVersions.filter((v) => serverVersions.includes(v))
  if (shared.length === 0) {
    return null
  }
  return shared.sort((a, b) => compareVersion(b, a))[0]
}

/** Compare dotted numeric versions: positive if `a` > `b`, zero if equal, negative otherwise. */
export function compareVersion(a: string, b: string): number {
  const partsA = a.split('.').map((p) => Number.parseInt(p, 10) || 0)
  const partsB = b.split('.').map((p) => Number.parseInt(p, 10) || 0)
  const len = Math.max(partsA.length, partsB.length)
  for (let i = 0; i < len; i += 1) {
    const diff = (partsA[i] ?? 0) - (partsB[i] ?? 0)
    if (diff !== 0) {
      return diff
    }
  }
  return 0
}
