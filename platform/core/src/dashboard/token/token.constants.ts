export const WRITE_ACCESS = 'write' as const
export const READ_ACCESS = 'read' as const

// Gates token *validity* in validateToken (any known level authenticates).
// Write authorization is enforced separately: @TokenReadAccess() routes +
// READ-mode Neo4j sessions for read-only tokens.
export const ACCESS_WEIGHT = {
  [WRITE_ACCESS]: 1,
  [READ_ACCESS]: 1
}

export function canWrite(level: string): boolean {
  return level === WRITE_ACCESS
}
