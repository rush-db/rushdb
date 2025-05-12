import { RELATION_DIRECTION_IN, RELATION_DIRECTION_OUT } from '@/core/entity/entity.constants'
import { TEntityPropertiesNormalized } from '@/core/entity/model/entity.interface'
import { TPropertyType } from '@/core/property/property.types'
import { SearchDto } from '@/core/search/dto/search.dto'

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
  uniq?: boolean
}

export type TCreateRecordSchema = Record<string, TCreateRecordSchemaField>

type BulkUpdateRecords = {
  where: SearchDto
  data: {
    name: null
    fullName: { alias?: '$record'; field: 'name' }
  }
}
