import { db } from '../util/db.js'

export async function ignoreRelationshipPattern({ id }: { id: string }) {
  const result = await db.relationships.patterns.ignore(id)
  return result.data
}
