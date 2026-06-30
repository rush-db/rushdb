export type SchemaVectorIndex = {
  id: string
  sourceType: string
  similarityFunction: string
  dimensions: number
  status: string
  modelKey: string
}

export type SchemaProperty = {
  id: string
  name: string
  type: string
  isArray?: boolean
  min?: number | string
  max?: number | string
  values?: Array<string | number>
  recordsCount?: number
  /** Non-empty when one or more embedding indexes exist for this (label, property) pair. */
  vectorIndexes?: SchemaVectorIndex[]
}

export type SchemaRelationship = {
  label: string
  type: string
  direction: 'in' | 'out'
  count?: number
  properties?: Array<{
    name: string
    type: string
    min?: number | string
    max?: number | string
    values?: Array<string | number | boolean>
    relationshipsCount?: number
  }>
}

export type SchemaItem = {
  label: string
  count: number
  properties: SchemaProperty[]
  relationships: SchemaRelationship[]
}
