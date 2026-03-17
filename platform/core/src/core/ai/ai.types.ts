export type OntologyProperty = {
  id: string
  name: string
  type: string
  min?: number | string
  max?: number | string
  values?: Array<string | number>
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
