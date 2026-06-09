import type {
  PROPERTY_TYPE_BOOLEAN,
  PROPERTY_TYPE_DATETIME,
  PROPERTY_TYPE_NULL,
  PROPERTY_TYPE_NUMBER,
  PROPERTY_TYPE_STRING
} from '../common/constants.js'
import type { MaybeArray, RequireAtLeastOne } from './utils.js'
import type {
  BooleanValue,
  DatetimeObject,
  DatetimeValue,
  NullValue,
  NumberValue,
  PropertyType,
  StringValue
} from './value.js'

export type DatetimeExpression =
  | DatetimeValue
  | RequireAtLeastOne<
      Record<'$eq' | '$gt' | '$gte' | '$lt' | '$lte' | '$ne', DatetimeObject | DatetimeValue> &
        Record<'$in' | '$nin', Array<DatetimeObject | DatetimeValue>> &
        Record<'$exists', BooleanValue>
    >

export type BooleanExpression =
  | BooleanValue
  | RequireAtLeastOne<Record<'$eq' | '$ne', BooleanValue> & Record<'$exists', BooleanValue>>

export type NullExpression =
  | NullValue
  | RequireAtLeastOne<Record<'$eq' | '$ne', NullValue> & Record<'$exists', BooleanValue>>

export type NumberExpression =
  | NumberValue
  | RequireAtLeastOne<
      Record<'$eq' | '$gt' | '$gte' | '$lt' | '$lte' | '$ne', NumberValue> &
        Record<'$in' | '$nin', Array<NumberValue>> &
        Record<'$exists', BooleanValue>
    >

export type StringExpression =
  | StringValue
  | RequireAtLeastOne<
      Record<'$contains' | '$endsWith' | '$eq' | '$ne' | '$startsWith', StringValue> &
        Record<'$in' | '$nin', Array<StringValue>> &
        Record<'$exists', BooleanValue>
    >

export type TypeExpression = {
  $type: PropertyType
}

export type PropertyExpression =
  | BooleanExpression
  | DatetimeExpression
  | NullExpression
  | NumberExpression
  | StringExpression
  | TypeExpression

export type PropertyExpressionByType = {
  [PROPERTY_TYPE_BOOLEAN]: BooleanExpression | TypeExpression
  [PROPERTY_TYPE_DATETIME]: DatetimeExpression | TypeExpression
  [PROPERTY_TYPE_NULL]: NullExpression | TypeExpression
  [PROPERTY_TYPE_NUMBER]: NumberExpression | TypeExpression
  [PROPERTY_TYPE_STRING]: StringExpression | TypeExpression
}

// Logical Expressions
export type LogicalGrouping<T> = Partial<{
  $and: MaybeArray<T>
  $nor: MaybeArray<T>
  $not: MaybeArray<T>
  $or: MaybeArray<T>
  $xor: MaybeArray<T>
}>
