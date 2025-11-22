import { CreateEntityDto } from '@/core/entity/dto/create-entity.dto'

export type TImportRecordsRelation = {
  source: string
  target: string
  type?: string
}
export type TImportJsonPayload = Record<string, any> | Array<Record<string, any>>
export type TImportOptions = {
  suggestTypes?: boolean
  castNumberArraysToVectors?: boolean
  convertNumericValuesToNumbers?: boolean
  capitalizeLabels?: boolean
  relationshipType?: string
  returnResult?: boolean
  mergeStrategy?: 'append' | 'rewrite'
  mergeBy?: string[]
}

// Subset of PapaParse config we allow clients to control (explicit allow-list)
export type TImportCsvParseConfig = {
  delimiter?: string
  header?: boolean
  skipEmptyLines?: boolean | 'greedy'
  dynamicTyping?: boolean
  quoteChar?: string
  escapeChar?: string
  newline?: string
}

export type WithId<T> = T & {
  id: string
}

export type TImportQueue = {
  key: string
  value: TImportJsonPayload
  parentId?: string
  target: WithId<CreateEntityDto>
  skip?: boolean
} & TImportOptions
