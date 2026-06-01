import { db } from '../util/db.js'

export async function deleteRelationshipPattern({
  id,
  deleteExisting
}: {
  id: string
  deleteExisting?: boolean
}) {
  const result = await db.relationships.patterns.delete(id, { deleteExisting })
  return result.data
}
