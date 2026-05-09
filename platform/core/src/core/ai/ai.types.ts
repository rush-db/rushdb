export type OntologyVectorIndex = {
  id: string
  sourceType: string
  similarityFunction: string
  dimensions: number
  status: string
  modelKey: string
}

export type OntologyProperty = {
  id: string
  name: string
  type: string
  min?: number | string
  max?: number | string
  values?: Array<string | number>
  recordsCount?: number
  /** Non-empty when one or more embedding indexes exist for this (label, property) pair. */
  vectorIndexes?: OntologyVectorIndex[]
}

export type OntologyRelationship = {
  label: string
  type: string
  direction: 'in' | 'out'
}

export type OntologyItem = {
  label: string
  count: number
  properties: OntologyProperty[]
  relationships: OntologyRelationship[]
}
