import type { PropertySingleValue, PropertyType, DBRecord } from '@rushdb/javascript-sdk'

import type { GenericApiResponse } from '~/types'

import { SearchOperations } from './constants'

type SearchOperation<
  Option extends {
    operation: string
  }
> = {
  name: string
} & Option

export type EqualsOperation = SearchOperation<{
  operation: SearchOperations.Equals
  value: PropertySingleValue
}>

export type ExcludeOperation = SearchOperation<{
  operation: SearchOperations.NotEquals
  value: PropertySingleValue
}>

export type GreaterOperation = SearchOperation<{
  operation: SearchOperations.Greater
  value: number | string
}>

export type GreaterOrEqualOperation = SearchOperation<{
  operation: SearchOperations.GreaterOrEqual
  value: number | string
}>

export type LessOperation = SearchOperation<{
  operation: SearchOperations.Less
  value: number | string
}>

export type LessOrEqualOperation = SearchOperation<{
  operation: SearchOperations.LessOrEqual
  value: number | string
}>

export type AnySearchOperation =
  | EqualsOperation
  | ExcludeOperation
  | GreaterOperation
  | GreaterOrEqualOperation
  | LessOperation
  | LessOrEqualOperation

/** operation with filterId */
export type Filter = AnySearchOperation & { filterId: string }

export const getViableSearchOperations = (propertyType: PropertyType): Array<SearchOperations> => {
  switch (propertyType) {
    case 'boolean':
      return [SearchOperations.Equals, SearchOperations.NotEquals]
    case 'string':
      return [
        SearchOperations.Equals,
        SearchOperations.NotEquals,
        SearchOperations.Contains,
        SearchOperations.StartsWith,
        SearchOperations.EndsWith
      ]
    case 'datetime':
    case 'number':
      return [
        SearchOperations.Equals,
        SearchOperations.NotEquals,
        SearchOperations.Greater,
        SearchOperations.GreaterOrEqual,
        SearchOperations.Less,
        SearchOperations.LessOrEqual
      ]
    default:
      return []
  }
}

export const isViableSearchOperation = ({
  propertyType,
  searchOperation
}: {
  propertyType: PropertyType
  searchOperation: SearchOperations
}) => getViableSearchOperations(propertyType).includes(searchOperation)

export type SearchResponse = GenericApiResponse<DBRecord[]>
