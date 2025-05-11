import type {
  PROPERTY_TYPE_BOOLEAN,
  PROPERTY_TYPE_DATETIME,
  PROPERTY_TYPE_NULL,
  PROPERTY_TYPE_NUMBER,
  PROPERTY_TYPE_STRING,
  PROPERTY_TYPE_VECTOR
} from '../common/constants.js'
import type { MaybeArray, RequireAtLeastOne } from './utils.js'
import type {
  BooleanValue,
  DatetimeObject,
  DatetimeValue,
  NullValue,
  NumberValue,
  StringValue
} from './value.js'

export type DatetimeExpression =
  | DatetimeValue
  | RequireAtLeastOne<
      Record<'$gt' | '$gte' | '$lt' | '$lte' | '$ne', DatetimeObject | DatetimeValue> &
        Record<'$in' | '$nin', Array<DatetimeObject | DatetimeValue>>
    >

export type BooleanExpression = BooleanValue | Record<'$ne', BooleanValue>

export type NullExpression = NullValue | Record<'$ne', NullValue>

export type NumberExpression =
  | NumberValue
  | RequireAtLeastOne<
      Record<'$gt' | '$gte' | '$lt' | '$lte' | '$ne', NumberValue> &
        Record<'$in' | '$nin', Array<NumberValue>>
    >

export type StringExpression =
  | StringValue
  | RequireAtLeastOne<
      Record<'$contains' | '$endsWith' | '$ne' | '$startsWith', StringValue> &
        Record<'$in' | '$nin', Array<StringValue>>
    >

export type VectorSearchFn = 'jaccard' | 'overlap' | 'cosine' | 'pearson' | 'euclideanDistance' | 'euclidean'
// Value range               [0,1]     | [0,1]     | [-1,1]   | [-1,1]    | [0, Infinity)       | (0, 1]

export type VectorExpression = {
  $vector: {
    fn: `gds.similarity.${VectorSearchFn}`
    query: Array<number>
    threshold: number | RequireAtLeastOne<Record<'$gt' | '$gte' | '$lt' | '$lte' | '$ne', number>>
  }
}

export type PropertyExpression =
  | BooleanExpression
  | DatetimeExpression
  | NullExpression
  | NumberExpression
  | StringExpression
  | VectorExpression

export type PropertyExpressionByType = {
  [PROPERTY_TYPE_BOOLEAN]: BooleanExpression
  [PROPERTY_TYPE_DATETIME]: DatetimeExpression
  [PROPERTY_TYPE_NULL]: NullExpression
  [PROPERTY_TYPE_NUMBER]: NumberExpression
  [PROPERTY_TYPE_STRING]: StringExpression
  [PROPERTY_TYPE_VECTOR]: VectorExpression
}

// Logical Expressions
export type LogicalGrouping<T> = Partial<{
  $and: MaybeArray<T>
  $nor: MaybeArray<T>
  $not: MaybeArray<T>
  $or: MaybeArray<T>
  $xor: MaybeArray<T>
}>
