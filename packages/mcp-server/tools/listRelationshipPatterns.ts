import { db } from '../util/db.js'

export async function listRelationshipPatterns() {
  const result = await db.relationships.patterns.list()
  return result.data
}
