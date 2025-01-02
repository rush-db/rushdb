import { TRelationDirection } from '@/core/entity/entity.types'
import { DatetimeObject, TPropertyType } from '@/core/property/property.types'

import { TSearchSort } from '../search/search.types'

export type RequireAtLeastOne<T> = {
  [K in keyof T]-?: Required<Pick<T, K>> & Partial<Pick<T, Exclude<keyof T, K>>>
}[keyof T]

export type MaybeArray<T> = Array<T> | T

export type FlatObject = Record<string, boolean | null | number | string | undefined>

export type Schema = Record<
  string,
  {
    required?: boolean
    type: TPropertyType
  }
>

// DATETIME
export type DatetimeValue = DatetimeObject | string
export type DatetimeExpression =
  | RequireAtLeastOne<
      Record<'$gt' | '$gte' | '$lt' | '$lte' | '$ne', DatetimeObject | string> &
        Record<'$in' | '$nin', Array<DatetimeObject | string>>
    >
  | DatetimeValue

// BOOLEAN
export type BooleanValue = boolean
export type BooleanExpression = Record<'$ne', BooleanValue> | BooleanValue

// NULL
export type NullValue = null
export type NullExpression = Record<'$ne', NullValue> | NullValue

// NUMBER
export type NumberValue = number
export type NumberExpression =
  | RequireAtLeastOne<
      Record<'$gt' | '$gte' | '$lt' | '$lte' | '$ne', NumberValue> &
        Record<'$in' | '$nin', Array<NumberValue>>
    >
  | NumberValue

// STRING
export type StringValue = string
export type StringExpression =
  | RequireAtLeastOne<
      Record<'$contains' | '$endsWith' | '$ne' | '$startsWith', StringValue> &
        Record<'$in' | '$nin', Array<StringValue>>
    >
  | StringValue

export type PropertyExpression =
  | BooleanExpression
  | DatetimeExpression
  | NullExpression
  | NumberExpression
  | StringExpression

export type PropertyExpressionByType = {
  boolean: BooleanExpression
  datetime: DatetimeExpression
  null: NullExpression
  number: NumberExpression
  string: StringExpression
}

export type LogicalGrouping<T> = {
  $and: MaybeArray<T>
  $not: MaybeArray<T>
  $or: MaybeArray<T>
  $xor: MaybeArray<T>
  $nor: MaybeArray<T>
}

export type Relation = string | { type: string; direction: TRelationDirection }

export type Related = {
  [K in keyof Models]?: {
    $relation?: Relation
    $alias?: string
  } & Where<Models[K]>
}

export type LogicalExpressionValue<T = PropertyExpression & Related> =
  | AndExpression<T>
  | NorExpression<T>
  | NotExpression<T>
  | OrExpression<T>
  | T
  | XorExpression<T>

export type AndExpression<T = PropertyExpression & Related> = {
  $and: MaybeArray<LogicalExpressionValue<T>>
}

export type OrExpression<T = PropertyExpression & Related> = {
  $or: MaybeArray<LogicalExpressionValue<T>>
}

export type NotExpression<T = PropertyExpression & Related> = {
  $not: MaybeArray<LogicalExpressionValue<T>>
}

export type XorExpression<T = PropertyExpression & Related> = {
  $xor: MaybeArray<LogicalExpressionValue<T>>
}

export type NorExpression<T = PropertyExpression & Related> = {
  $nor: MaybeArray<LogicalExpressionValue<T>>
}

export type LogicalExpression<T = PropertyExpression & Related> = RequireAtLeastOne<
  AndExpression<T> & OrExpression<T> & NotExpression<T> & XorExpression<T> & NorExpression<T>
>

type MaybeLogicalExpression<T> = LogicalExpression<T> | T

type SchemaBasedExpression<T> =
  T extends Schema ? MaybeLogicalExpression<PropertyExpressionByType[T[keyof T]['type']]> : never

type ObjectBasedExpression<T> =
  T extends number ? MaybeLogicalExpression<NumberExpression>
  : T extends boolean ? MaybeLogicalExpression<BooleanExpression>
  : T extends string ? MaybeLogicalExpression<StringExpression> | MaybeLogicalExpression<DatetimeExpression>
  : T extends null ? MaybeLogicalExpression<NullExpression>
  : LogicalExpression | Partial<PropertyExpression & Related>

export type Condition<T extends FlatObject | Schema = Schema> =
  | {
      [K in keyof T]?: SchemaBasedExpression<T> | ObjectBasedExpression<T[K]>
    }
  | { $id?: MaybeLogicalExpression<StringExpression> }

export type Where<T extends FlatObject | Schema = Schema> = (Condition<T> & Related) &
  Partial<LogicalGrouping<Condition<T> & Related>>

export type AggregateCollectFn = {
  skip?: number
  alias: string
  limit?: number
  field?: string
  fn: 'collect'
  orderBy?: TSearchSort
  uniq?: boolean
}

export type AggregateCollectNestedFn = Omit<AggregateCollectFn, 'field'> & {
  aggregate?: { [field: string]: AggregateCollectNestedFn }
}

export type AliasesMap = Record<string, string>

export type AggregateFn<S extends Schema = Schema> =
  | { field: string; fn: 'avg'; alias: string; precision?: number }
  | { field?: string; fn: 'count'; uniq?: boolean; alias: string }
  | { field: string; fn: 'max'; alias: string }
  | { field: string; fn: 'min'; alias: string }
  | { field: string; fn: 'sum'; alias: string }
  | AggregateCollectFn

export type Aggregate =
  | {
      [field: string]: string | AggregateFn
    }
  | {
      [field: string]: AggregateCollectNestedFn
    }

/* Extend this type */
// eslint-disable-next-line @typescript-eslint/no-empty-interface
export interface Models {}
