import type { Model, RelationOptions } from '../sdk/index.js'
import type {
  BooleanExpression,
  DatetimeExpression,
  LogicalGrouping,
  NullExpression,
  NumberExpression,
  PropertyExpression,
  PropertyExpressionByType,
  StringExpression,
  VectorSearchFn
} from './expressions.js'
import type { Schema } from './schema.js'
import type { AnyObject, MaybeArray, RequireAtLeastOne } from './utils.js'

export type Related<M extends Record<string, Model['schema']> = Models> =
  keyof M extends never ? AnyObject
  : {
      [Key in keyof M]?: {
        $alias?: string
        $relation?: RelationOptions | string
      } & Where<M[Key]>
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

export type IdExpression = MaybeLogicalExpression<
  { $id?: LogicalExpression<StringExpression> | StringExpression } & Related
>

export type Expression<S extends Schema = Schema> =
  | IdExpression
  | (S extends Schema ?
      // When used with actual Schema
      {
        [Key in keyof S]?: MaybeLogicalExpression<PropertyExpressionByType[S[Key]['type']]>
      }
    : // When used with random object
      {
        [Key in keyof S]?: S[Key] extends MaybeArray<number> ? MaybeLogicalExpression<NumberExpression>
        : S[Key] extends MaybeArray<boolean> ? MaybeLogicalExpression<BooleanExpression>
        : S[Key] extends MaybeArray<string> ?
          MaybeLogicalExpression<DatetimeExpression> | MaybeLogicalExpression<StringExpression>
        : S[Key] extends MaybeArray<null> ? MaybeLogicalExpression<NullExpression>
        : LogicalExpression | Partial<PropertyExpression>
      })

export type Where<S extends Schema = Schema> =
  | ((Expression<S> & Related) & LogicalGrouping<Expression<S> & Related>)
  | LogicalGrouping<Expression<S> & Related>

export type OrderDirection = 'asc' | 'desc'
export type Order<S extends Schema = Schema> = OrderDirection | Partial<Record<keyof S, OrderDirection>>

export type AggregateCollectFn = {
  alias: string
  field?: string
  fn: 'collect'
  limit?: number
  orderBy?: Order
  skip?: number
  unique?: boolean
}

export type AggregateCollectNestedFn = Omit<AggregateCollectFn, 'field'> & {
  aggregate?: { [field: string]: AggregateCollectNestedFn }
}

export type AggregateCountFn = { field?: string; fn: 'count'; unique?: boolean; alias?: string }

export type AggregateTimeBucketFn = {
  field: string
  fn: 'timeBucket'
  alias?: string
  granularity: 'day' | 'week' | 'month' | 'quarter' | 'year' | 'months'
  // When granularity === 'months', size (>0) defines the number of months per bucket (e.g. 2, 3, 6, 12)
  size?: number
}

// eslint-disable-next-line @typescript-eslint/no-unused-vars
export type AggregateFn<S extends Schema = Schema> =
  | { field: string; fn: 'avg'; alias?: string; precision?: number }
  | AggregateCountFn
  | { field: string; fn: 'max'; alias?: string }
  | { field: string; fn: 'min'; alias?: string }
  | { field: string; fn: 'sum'; alias?: string }
  | { field: string; fn: `gds.similarity.${VectorSearchFn}`; alias?: string; query: number[] }
  | AggregateTimeBucketFn
  | AggregateCollectFn

export type Aggregate =
  | {
      [field: string]: AggregateCollectNestedFn
    }
  | {
      [field: string]: AggregateFn | string
    }

type InferAggregateType<T extends AggregateFn | string> =
  T extends string ? any
  : T extends { fn: 'sum' | 'avg' | 'min' | 'max' | 'count' } ? number
  : T extends { fn: 'collect' } ? Array<any>
  : never

// Helper type to extract aggregated fields
export type ExtractAggregateFields<A extends Record<string, AggregateFn>> = {
  [K in keyof A]: InferAggregateType<A[K]>
}

// CLAUSES
export type WhereClause<S extends Schema = Schema> = {
  where?: Where<S>
}

export type PaginationClause = {
  limit?: number
  skip?: number
}

export type SearchQueryLabelsClause = {
  labels?: Array<string>
}

export type OrderClause<S extends Schema = Schema> = {
  orderBy?: Order<S>
}

export type AggregateClause = {
  aggregate?: Aggregate
}

export type GroupByClause = {
  groupBy?: Array<string>
}

export type SearchQuery<S extends Schema = any> = SearchQueryLabelsClause &
  PaginationClause &
  OrderClause<S> &
  WhereClause<S> &
  AggregateClause &
  GroupByClause

/** Redeclare Models type in order to have suggestions over related records fields **/
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface Models {}
