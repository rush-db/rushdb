import type {
  CollectPropertySingleValue,
  CollectPropertyType,
  CollectRecord
} from '@collect.so/javascript-sdk'

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
  value: CollectPropertySingleValue
}>

// export type ExactMatchOperation = SearchOperation<{
//   operation: SearchOperations.ExactMatch
//   value: PropertyValue
// }>

export type ExcludeOperation = SearchOperation<{
  operation: SearchOperations.NotEquals
  value: CollectPropertySingleValue
}>

// export type ExactExcludeOperation = SearchOperation<{
//   operation: SearchOperations.ExactExclude
//   value: PropertyValue
// }>
//
// export type RangeOperation = SearchOperation<{
//   max?: ISO8601 | number
//   min?: ISO8601 | number
//   operation: SearchOperations.Range
// }>
//
// export type ExcludeRangeOperation = SearchOperation<{
//   max: number
//   min: number
//   operation: SearchOperations.ExcludeRange
// }>

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
  // | ExactExcludeOperation
  // | ExactMatchOperation
  | EqualsOperation
  // | ExcludeRangeOperation
  | ExcludeOperation
  | GreaterOperation
  | GreaterOrEqualOperation
  | LessOperation
  | LessOrEqualOperation
// | RangeOperation

/** operation with filterId */
export type Filter = AnySearchOperation & { filterId: string }

// export const isNumberRangeOperation = (
//   operation: Pick<AnySearchOperation, 'operation'>
// ): operation is ExcludeRangeOperation | RangeOperation => {
//   switch (operation.operation) {
//     case SearchOperations.Range:
//     case SearchOperations.ExcludeRange:
//       return true
//     default:
//       return false
//   }
// }

export const getViableSearchOperations = (
  propertyType: CollectPropertyType
): Array<SearchOperations> => {
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
  propertyType: CollectPropertyType
  searchOperation: SearchOperations
}) => getViableSearchOperations(propertyType).includes(searchOperation)

export type SearchResponse = GenericApiResponse<CollectRecord[]>
