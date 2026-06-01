import { db } from '../util/db.js'

export async function analyzeRelationshipPatterns() {
  const result = await db.relationships.patterns.analyze()
  return result.data
}
