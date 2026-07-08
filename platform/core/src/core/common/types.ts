import { TRelationDirection } from '@/core/entity/entity.types'
import { DatetimeObject, TPropertyType } from '@/core/property/property.types'

import { TSearchSort } from '../search/search.types'

export type RequireAtLeastOne<T> = {
  [K in keyof T]-?: Required<Pick<T, K>> & Partial<Pick<T, Exclude<keyof T, K>>>
}[keyof T]

export type MaybeArray<T> = Array<T> | T

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
        Record<'$in' | '$nin', Array<DatetimeObject | string>> &
        Record<'$exists', BooleanValue>
    >
  | DatetimeValue

// BOOLEAN
export type BooleanValue = boolean
export type BooleanExpression = Record<'$ne' | '$exists', BooleanValue> | BooleanValue

// NULL
export type NullValue = null
export type NullExpression = Record<'$ne' | '$exists', NullValue | BooleanValue> | NullValue

// NUMBER
export type NumberValue = number
export type NumberExpression =
  | RequireAtLeastOne<
      Record<'$gt' | '$gte' | '$lt' | '$lte' | '$ne', NumberValue> &
        Record<'$in' | '$nin', Array<NumberValue>> &
        Record<'$exists', BooleanValue>
    >
  | NumberValue

// STRING
export type StringValue = string
export type StringExpression =
  | RequireAtLeastOne<
      Record<'$contains' | '$endsWith' | '$ne' | '$startsWith', StringValue> &
        Record<'$in' | '$nin', Array<StringValue>> &
        Record<'$exists', BooleanValue>
    >
  | StringValue

// TYPE
export type TypeExpression = Record<'$type', TPropertyType>

export type PropertyExpression =
  | BooleanExpression
  | DatetimeExpression
  | NullExpression
  | NumberExpression
  | StringExpression
  | TypeExpression

export type PropertyExpressionByType = {
  boolean: BooleanExpression | TypeExpression
  datetime: DatetimeExpression | TypeExpression
  null: NullExpression | TypeExpression
  number: NumberExpression | TypeExpression
  string: StringExpression | TypeExpression
}

export type LogicalGrouping<T> = {
  $and: MaybeArray<T>
  $not: MaybeArray<T>
  $or: MaybeArray<T>
  $xor: MaybeArray<T>
  $nor: MaybeArray<T>
}

export type TraversalHops = number | { min?: number; max?: number }

export type Relation = string | { type?: string; direction?: TRelationDirection; hops?: TraversalHops }

/** Record-level cycle predicate: the record lies on a closed path over these edges.
 *  `hops` is mandatory (floor 2); intermediate node labels are unconstrained. */
export type CycleExpression = { type?: string; direction?: TRelationDirection; hops: TraversalHops }

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

export type Condition<T extends Schema = Schema> =
  | {
      [K in keyof T]?: SchemaBasedExpression<T> | ObjectBasedExpression<T[K]>
    }
  | { $id?: MaybeLogicalExpression<StringExpression> }

export type Where<T extends Schema = Schema> = (Condition<T> & Related & { $cycle?: CycleExpression }) &
  Partial<LogicalGrouping<Condition<T> & Related & { $cycle?: CycleExpression }>>

/** @deprecated Use SelectExprMap / select expressions instead. To be removed in a future major version. */
export type AggregateCollectFn = {
  skip?: number
  alias: string
  limit?: number
  field?: string
  fn: 'collect'
  orderBy?: TSearchSort
  unique?: boolean
}

/** @deprecated Use SelectExprMap / select expressions instead. To be removed in a future major version. */
export type AggregateCollectNestedFn = Omit<AggregateCollectFn, 'field'> & {
  aggregate?: { [field: string]: AggregateCollectNestedFn }
}

export type AliasesMap = Record<string, string>

/** @deprecated Use SelectExprMap / select expressions instead. To be removed in a future major version. */
export type AggregateCountFn = {
  field?: string
  fn: 'count'
  unique?: boolean
  /** Defaults to '$record' */ alias?: string
}

/** @deprecated Use SelectExprMap / select expressions instead. To be removed in a future major version. */
export type AggregateTimeBucketFn = {
  field: string
  fn: 'timeBucket'
  alias?: string
  granularity:
    | 'day'
    | 'week'
    | 'month'
    | 'quarter'
    | 'year'
    | 'months'
    | 'hour'
    | 'minute'
    | 'second'
    | 'hours'
    | 'minutes'
    | 'seconds'
    | 'years'
  // When granularity === 'months' | 'hours' | 'minutes' | 'seconds', size (>0) defines the number of units per bucket (e.g. months: 2, 3, 6; hours: 1, 6, 12; minutes: 5, 15; seconds: 10, 30)
  size?: number
}

/** @deprecated Use SelectExprMap / select expressions instead. To be removed in a future major version. */
export type AggregateFn<S extends Schema = Schema> =
  | { field: string; fn: 'avg'; alias?: string; precision?: number }
  | AggregateCountFn
  | { field: string; fn: 'max'; alias?: string }
  | { field: string; fn: 'min'; alias?: string }
  | { field: string; fn: 'sum'; alias?: string }
  | {
      field: string
      fn: 'vector.similarity.cosine' | 'vector.similarity.euclidean'
      alias?: string
      query: number[]
    }
  | AggregateTimeBucketFn
  | AggregateCollectFn

/** @deprecated Use SelectExprMap / select expressions instead. To be removed in a future major version. */
export type Aggregate =
  | {
      [field: string]: string | AggregateFn
    }
  | {
      [field: string]: AggregateCollectNestedFn
    }

// ── Select Expression System ───────────────────────────────────────────────

export type CollectExpr = {
  /** Reference a pre-declared $alias from the where clause. Mutually exclusive with `label`. */
  from?: string
  /** Inline relation traversal — no $alias in where needed. Mutually exclusive with `from`. */
  label?: string
  /** Per-level filter (only valid with `label`). Flat property conditions only. */
  where?: Record<string, any>
  select?: SelectExprMap
  orderBy?: TSearchSort
  limit?: number
  skip?: number
  unique?: boolean
}

export type TimeBucketExprUnit =
  | 'day'
  | 'week'
  | 'month'
  | 'quarter'
  | 'year'
  | 'months'
  | 'hour'
  | 'minute'
  | 'second'
  | 'hours'
  | 'minutes'
  | 'seconds'
  | 'years'

export type TimeBucketExpr = {
  /** Field reference, e.g. "$record.createdAt" */
  field: string
  unit: TimeBucketExprUnit
  /** Required when unit is months/hours/minutes/seconds/years */
  size?: number
}

export type Expr =
  | string // "$record.field" field ref, or "$alias" alias ref
  | number // literal number
  | boolean // literal boolean
  | { $ref: string } // cross-expression reference
  | { $sum: Expr }
  | { $avg: Expr; $precision?: number }
  | { $count: '*' | Expr }
  | { $min: Expr }
  | { $max: Expr }
  | { $divide: [Expr, Expr] }
  | { $multiply: [Expr, Expr] }
  | { $add: [Expr, Expr] }
  | { $subtract: [Expr, Expr] }
  | { $collect: CollectExpr }
  | { $timeBucket: TimeBucketExpr }

/** Canonical output-shaping clause — replaces `aggregate` */
export type SelectExprMap = Record<string, Expr>

/* Extend this type */
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface Models {}
