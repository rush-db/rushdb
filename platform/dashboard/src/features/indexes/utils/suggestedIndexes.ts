import type { EmbeddingIndex } from '~/features/indexes/types'

export type SchemaProperty = {
  id: string
  name: string
  type: string
  values?: Array<string | number>
  recordsCount?: number
}

export type SchemaItem = {
  label: string
  count: number
  properties: SchemaProperty[]
}

export type LabelProperty = {
  label: string
  property: SchemaProperty
}

export type SuggestedEmbeddingIndex = {
  label: string
  propertyName: string
  recordsCount?: number
  reason: string
  score: number
}

const TEXT_VALUE_LENGTH_THRESHOLD = 16

function indexKey(label: string, propertyName: string) {
  return `${label}:${propertyName}`
}

function isTextProperty(property: SchemaProperty) {
  return property.type === 'string' || property.type === 'text'
}

// Identifier/reference fields (e.g. `faction_id`, `related_event_ids`, `eventId`, bare
// `id`/`ids`) hold opaque keys, not natural language, so they're poor semantic-search
// candidates even when their values are long. Exclude them by name.
function isIdLikeProperty(name: string) {
  // snake_case or exact: id, ids, faction_id, related_event_ids
  if (/(^|_)ids?$/.test(name.toLowerCase())) return true
  // camelCase: factionId, eventIds
  if (/[a-z]Ids?$/.test(name)) return true
  return false
}

function getTextValueStats(values: SchemaProperty['values']) {
  const lengths = (values ?? [])
    .filter((value): value is string => typeof value === 'string')
    .map((value) => value.trim().length)
    .filter((length) => length > 0)

  const longValueCount = lengths.filter((length) => length > TEXT_VALUE_LENGTH_THRESHOLD).length
  const maxLength = lengths.length > 0 ? Math.max(...lengths) : 0

  return {
    longValueCount,
    maxLength
  }
}

export function flattenSchemaProperties(schema: SchemaItem[]): LabelProperty[] {
  return schema.flatMap((item) =>
    item.properties.map((property) => ({
      label: item.label,
      property
    }))
  )
}

export function buildSuggestedEmbeddingIndexes({
  existingIndexes,
  properties
}: {
  existingIndexes?: EmbeddingIndex[]
  properties: LabelProperty[]
}): SuggestedEmbeddingIndex[] {
  const indexedPairs = new Set(
    existingIndexes
      ?.filter((index) => index.sourceType === 'managed')
      .map((index) => indexKey(index.label, index.propertyName))
  )
  const byPair = new Map<string, SuggestedEmbeddingIndex>()

  properties.forEach(({ label, property }) => {
    if (!isTextProperty(property)) return
    if (isIdLikeProperty(property.name)) return
    if (property.recordsCount === 0) return
    if (indexedPairs.has(indexKey(label, property.name))) return

    const { longValueCount, maxLength } = getTextValueStats(property.values)
    if (longValueCount === 0) return

    const score = Math.min(maxLength, 240) + longValueCount * 20 + Math.min(property.recordsCount ?? 0, 20)
    const key = indexKey(label, property.name)
    const current = byPair.get(key)
    const candidate = {
      label,
      propertyName: property.name,
      recordsCount: property.recordsCount,
      reason: `Sample values include text longer than ${TEXT_VALUE_LENGTH_THRESHOLD} characters`,
      score
    } satisfies SuggestedEmbeddingIndex

    if (!current || candidate.score > current.score) {
      byPair.set(key, candidate)
    }
  })

  return Array.from(byPair.values()).sort((a, b) => {
    if (b.score !== a.score) return b.score - a.score
    if ((b.recordsCount ?? 0) !== (a.recordsCount ?? 0)) return (b.recordsCount ?? 0) - (a.recordsCount ?? 0)
    return `${a.label}:${a.propertyName}`.localeCompare(`${b.label}:${b.propertyName}`)
  })
}
