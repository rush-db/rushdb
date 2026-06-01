import { db } from '../util/db.js'

export async function approveRelationshipPattern({ id }: { id: string }) {
  const result = await db.relationships.patterns.approve(id)
  return result.data
}
