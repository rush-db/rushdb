import { CreateEntityDto } from '@/core/entity/dto/create-entity.dto'

export type TImportRecordsRelation = {
  source: string
  target: string
  type?: string
  properties?: Record<string, unknown>
}
export type TImportJsonPayload = Record<string, any> | Array<Record<string, any>>
export type TImportJsonInputFormat = 'json' | 'jsonl' | 'ndjson'
export type TImportOptions = {
  suggestTypes?: boolean
  convertNumericValuesToNumbers?: boolean
  capitalizeLabels?: boolean
  /**
   * When true, treats empty values as unset: empty strings ('') and empty arrays ([]) are skipped
   * and no property is created for them (an array's '' elements are stripped first; an array that
   * becomes empty is dropped). Zero (0) and false are real values and are never affected.
   * Defaults to false — empty strings/arrays are stored as-is.
   */
  skipEmptyValues?: boolean
  relationshipType?: string
  /**
   * When true, returns the imported records in the response body.
   * For imports exceeding 1000 records, this option is silently ignored
   * and a summary response is returned instead.
   */
  returnResult?: boolean
  mergeStrategy?: 'append' | 'rewrite'
  mergeBy?: string[]
}

export type TImportSummary = {
  message: string
  count: number
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
