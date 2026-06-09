import type { EmbeddingIndex } from '~/features/indexes/types'

export type OntologyProperty = {
  id: string
  name: string
  type: string
  values?: Array<string | number>
  recordsCount?: number
}

export type OntologyItem = {
  label: string
  count: number
  properties: OntologyProperty[]
}

export type LabelProperty = {
  label: string
  property: OntologyProperty
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

function isTextProperty(property: OntologyProperty) {
  return property.type === 'string' || property.type === 'text'
}

function getTextValueStats(values: OntologyProperty['values']) {
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

export function flattenOntologyProperties(ontology: OntologyItem[]): LabelProperty[] {
  return ontology.flatMap((item) =>
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
