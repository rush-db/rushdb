import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_LABEL,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META
} from '@/core/common/constants'
import { RELATION_DIRECTION_IN, RELATION_DIRECTION_OUT } from '@/core/entity/entity.constants'
import { TPropertyType, TPropertyValue } from '@/core/property/property.types'
import { SearchDto } from '@/core/search/dto/search.dto'

export type TEntityPropertiesNormalized = {
  [RUSHDB_KEY_ID]: string
  [RUSHDB_KEY_PROJECT_ID]: string
  [RUSHDB_KEY_LABEL]?: string
  [RUSHDB_KEY_PROPERTIES_META]?: string
} & Partial<{
  [key: string]: TPropertyValue
}>

export type TRecordSearchResult = {
  total: number
  data: TEntityPropertiesNormalized[]
}

export type TRecordRelationsResponse = {
  total: number
  data: Array<{
    sourceId: string
    sourceLabel: string
    targetId: string
    targetLabel: string
    type: string
  }>
}

export type TRelationDirection = typeof RELATION_DIRECTION_IN | typeof RELATION_DIRECTION_OUT

type TCreateRecordSchemaField<T extends TPropertyType = TPropertyType> = {
  multiple?: boolean
  required?: boolean
  type: T
  unique?: boolean
}

export type TCreateRecordSchema = Record<string, TCreateRecordSchemaField>

type BulkUpdateRecords = {
  where: SearchDto
  data: {
    name: null
    fullName: { alias?: '$record'; field: 'name' }
  }
}
