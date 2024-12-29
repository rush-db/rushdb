import { RELATION_DIRECTION_IN, RELATION_DIRECTION_OUT } from '@/core/entity/entity.constants'
import { TEntityPropertiesNormalized } from '@/core/entity/model/entity.interface'

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
