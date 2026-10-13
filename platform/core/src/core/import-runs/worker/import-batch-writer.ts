import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_LABEL_PROPERTY,
  RUSHDB_LABEL_RECORD,
  RUSHDB_RELATION_VALUE
} from '@/core/common/constants'

/** Minimal transaction surface shared by Neogma transactions and test doubles. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export interface GraphTxLike {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  run(query: string, params?: Record<string, any>): Promise<any>
}

export interface PropertyDraft {
  name: string
  value?: unknown
  type?: string
  id?: string
  created?: string
  metadata?: string
}

export interface RecordDraft {
  id: string
  label: string
  properties: PropertyDraft[]
}

export interface RelationDraft {
  source: string
  target: string
  type: string
  properties?: Record<string, unknown>
}

const IDENTIFIER_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/

function escapeBacktickIdentifier(raw: string): string {
  return `\`${String(raw).replace(/`/g, '``')}\``
}

function assertSafeIdentifier(raw: string, kind: string): string {
  if (!IDENTIFIER_PATTERN.test(raw)) {
    throw new Error(`invalid ${kind}: ${raw}`)
  }
  return raw
}

/**
 * Detects replay-ID collisions against pre-existing records owned by other
 * projects. Deterministic IDs already embed the project ID, so a hit here means
 * an incompatible legacy node occupies the identity space and must fail loudly.
 */
export function buildCollisionCheckQuery(): string {
  const queryBuilder = [
    `UNWIND $ids as candidateId`,
    `OPTIONAL MATCH (existing:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: candidateId })`,
    `WITH existing WHERE existing IS NOT NULL AND existing.${RUSHDB_KEY_PROJECT_ID} <> $projectId`,
    `RETURN count(existing) as collisions`
  ]
  return queryBuilder.join('\n')
}

export async function assertNoReplayCollisions(
  tx: GraphTxLike,
  params: { projectId: string; recordIds: string[] }
): Promise<void> {
  if (params.recordIds.length === 0) {
    return
  }

  const result = await tx.run(buildCollisionCheckQuery(), {
    projectId: params.projectId,
    ids: params.recordIds
  })
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const row = result?.records?.[0]?.toObject?.()
  const collisions = Number(row?.collisions ?? 0)
  if (collisions > 0) {
    throw new Error(`IMPORT_REPLAY_ID_COLLISION: ${collisions} conflicting record(s)`)
  }
}

/**
 * Idempotent batch MERGE of import records keyed by the deterministic replay ID.
 * Business labels, __proptypes meta, and Property nodes follow exactly the
 * conventions of the synchronous import path (EntityQueryService.importRecords +
 * processProps), so async output is graph-compatible.
 */
export function buildMergeRecordsQuery(): string {
  return [
    `WITH $records as recordsToMerge, datetime() as time`,
    `UNWIND recordsToMerge as r`,
    `MERGE (record:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: r.id })`,
    `ON CREATE SET record.${RUSHDB_KEY_PROJECT_ID} = $projectId`,
    `WITH record, r, time,`,
    `apoc.map.fromPairs([property IN r.properties | [property.name, coalesce(property.type, 'string')]]) AS typesMap,`,
    `apoc.map.fromPairs([property IN r.properties | [property.name, property.value]]) AS valuesMap`,
    `CALL apoc.create.addLabels(record, [r.label]) YIELD node as labeledRecord`,
    `SET labeledRecord.${RUSHDB_KEY_PROPERTIES_META} = apoc.convert.toJson(typesMap)`,
    `SET labeledRecord += valuesMap`,
    `WITH DISTINCT labeledRecord as record, r.properties as props, time`,
    `UNWIND props as prop`,
    `MERGE (p:${RUSHDB_LABEL_PROPERTY} { name: prop.name, type: coalesce(prop.type, 'string'), ${RUSHDB_KEY_PROJECT_ID}: $projectId, metadata: coalesce(prop.metadata, "") })`,
    `ON CREATE SET p.created = coalesce(prop.created, time), p.id = prop.id, p.metadata = coalesce(prop.metadata, "")`,
    `MERGE (p)-[rel:${RUSHDB_RELATION_VALUE}]->(record)`,
    `RETURN count(DISTINCT record) as mergedRecords`
  ].join('\n')
}

export async function mergeRecords(
  tx: GraphTxLike,
  params: { projectId: string; records: RecordDraft[] }
): Promise<number> {
  await assertNoReplayCollisions(tx, {
    projectId: params.projectId,
    recordIds: params.records.map((r) => r.id)
  })

  const result = await tx.run(buildMergeRecordsQuery(), {
    projectId: params.projectId,
    records: params.records
  })
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const row = result?.records?.[0]?.toObject?.()
  return Number(row?.mergedRecords ?? 0)
}

/**
 * Idempotent relationship creation grouped by one static type per call.
 * Identical endpoint pairs collapse into a single typed relationship, which makes
 * crash-boundary replays converge instead of duplicating edges.
 */
export function buildMergeRelationsQuery(relationshipType: string): string {
  const safeType = escapeBacktickIdentifier(assertSafeIdentifier(relationshipType, 'relationship type'))

  return [
    `WITH $relations as relations`,
    `UNWIND relations as relation`,
    `MATCH (source:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: relation.source, ${RUSHDB_KEY_PROJECT_ID}: $projectId })`,
    `MATCH (target:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: relation.target, ${RUSHDB_KEY_PROJECT_ID}: $projectId })`,
    `WHERE NOT EXISTS { MATCH (source)-[anyExisting:${safeType}]->(target) }`,
    `CREATE (source)-[createdRel:${safeType}]->(target)`,
    `SET createdRel += coalesce(relation.properties, {})`,
    `RETURN count(createdRel) as createdRelations`
  ].join('\n')
}

export async function linkRelations(
  tx: GraphTxLike,
  params: { projectId: string; relations: RelationDraft[] }
): Promise<number> {
  const byType = new Map<string, RelationDraft[]>()
  for (const relation of params.relations) {
    const list = byType.get(relation.type) ?? []
    list.push(relation)
    byType.set(relation.type, list)
  }

  let total = 0
  for (const [type, relations] of byType) {
    const result = await tx.run(buildMergeRelationsQuery(type), {
      projectId: params.projectId,
      relations
    })
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const row = result?.records?.[0]?.toObject?.()
    total += Number(row?.createdRelations ?? 0)
  }
  return total
}

/**
 * Resolves endpoint values to persisted record IDs for link files. Values are
 * matched as strings against `label.keyProperty` so numeric CSV typing on either
 * side cannot break resolution.
 */
export async function resolveEndpoints(
  tx: GraphTxLike,
  params: { projectId: string; label: string; keyProperty: string; values: string[] }
): Promise<Map<string, string>> {
  const safeLabel = escapeBacktickIdentifier(params.label)

  const query = [
    `UNWIND $entries as entry`,
    `MATCH (node:${RUSHDB_LABEL_RECORD}:${safeLabel})`,
    `WHERE node.${RUSHDB_KEY_PROJECT_ID} = $projectId AND toString(node[entry.key]) = entry.value`,
    `RETURN toString(node.${RUSHDB_KEY_ID}) as id, entry.value as value`
  ].join('\n')

  const entries = params.values.map((value) => ({ key: params.keyProperty, value }))
  const resolved = new Map<string, string>()

  const result = await tx.run(query, { projectId: params.projectId, entries })
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  for (const record of result?.records ?? []) {
    const row = record.toObject()
    resolved.set(String(row.value), String(row.id))
  }

  return resolved
}

export async function hasAnyRecordForLabel(
  tx: GraphTxLike,
  params: { projectId: string; label: string }
): Promise<boolean> {
  const safeLabel = escapeBacktickIdentifier(params.label)
  const result = await tx.run(
    `MATCH (node:${RUSHDB_LABEL_RECORD}:${safeLabel}) WHERE node.${RUSHDB_KEY_PROJECT_ID} = $projectId RETURN node.${RUSHDB_KEY_ID} as id LIMIT 1`,
    { projectId: params.projectId }
  )
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return (result?.records?.length ?? 0) > 0
}

export interface LinkPair {
  sourceValue: string
  targetValue: string
  sourceId?: string
  targetId?: string
  properties: Record<string, unknown>
}

/**
 * Creates link-file relationships between resolved endpoints, idempotently,
 * grouped under one static relationship type.
 */
export function buildCreateLinksQuery(relationshipType: string): string {
  const safeType = escapeBacktickIdentifier(assertSafeIdentifier(relationshipType, 'relationship type'))

  return [
    `WITH $pairs as pairs`,
    `UNWIND pairs as pair`,
    `MATCH (source:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: pair.sourceId, ${RUSHDB_KEY_PROJECT_ID}: $projectId })`,
    `MATCH (target:${RUSHDB_LABEL_RECORD} { ${RUSHDB_KEY_ID}: pair.targetId, ${RUSHDB_KEY_PROJECT_ID}: $projectId })`,
    `WHERE NOT EXISTS { MATCH (source)-[anyExisting:${safeType}]->(target) }`,
    `CREATE (source)-[createdRel:${safeType}]->(target)`,
    `SET createdRel += coalesce(pair.properties, {})`,
    `RETURN count(createdRel) as createdLinks`
  ].join('\n')
}

export async function createLinks(
  tx: GraphTxLike,
  params: { projectId: string; relationshipType: string; pairs: LinkPair[] }
): Promise<{ created: number }> {
  const resolvable = params.pairs.filter((p) => p.sourceId && p.targetId)
  if (resolvable.length === 0) {
    return { created: 0 }
  }

  const result = await tx.run(buildCreateLinksQuery(params.relationshipType), {
    projectId: params.projectId,
    pairs: resolvable
  })
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const row = result?.records?.[0]?.toObject?.()
  return { created: Number(row?.createdLinks ?? 0) }
}
