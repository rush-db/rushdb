import type { Model, RelationOptions } from '../sdk/index.js'
import type {
  BooleanExpression,
  DatetimeExpression,
  LogicalGrouping,
  NullExpression,
  NumberExpression,
  PropertyExpression,
  PropertyExpressionByType,
  StringExpression
} from './expressions.js'
import type { Schema } from './schema.js'
import type { AnyObject, MaybeArray, RequireAtLeastOne } from './utils.js'

/**
 * Variable-length traversal depth for `$relation.hops` and `$cycle.hops`.
 * A number matches exactly that many hops; `{ min?, max? }` matches a range
 * (`min` defaults to 1, or 2 for `$cycle`). Omitting `max` requests
 * unbounded traversal, which is only allowed on self-hosted deployments and
 * projects with a custom Neo4j instance.
 */
export type TraversalHops = number | { min?: number; max?: number }

/**
 * Relationship constraints for a traversal block in `where`.
 * Extends {@link RelationOptions} with variable-length traversal support.
 */
export type TraversalRelationOptions = RelationOptions & {
  hops?: TraversalHops
}

/**
 * Record-level cycle predicate: matches records sitting on a closed path
 * (cycle/ring) back to themselves over the described edges. The value is the
 * traversal spec itself — `hops` is mandatory (`min` ≥ 2, defaults to 2);
 * intermediate node labels are unconstrained. A cycle has no separate endpoint
 * to filter or alias.
 */
export type CycleExpression = RelationOptions & {
  hops: TraversalHops
}

export type Related<M extends Record<string, Model['schema']> = Models> =
  keyof M extends never ? AnyObject
  : {
      [Key in keyof M]?: {
        $alias?: string
        $relation?: TraversalRelationOptions | string
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
  | ((Expression<S> & Related & { $cycle?: CycleExpression }) &
      LogicalGrouping<Expression<S> & Related & { $cycle?: CycleExpression }>)
  | LogicalGrouping<Expression<S> & Related & { $cycle?: CycleExpression }>

export type OrderDirection = 'asc' | 'desc'
export type Order<S extends Schema = Schema> = OrderDirection | Partial<Record<keyof S, OrderDirection>>

/**
 * @deprecated The aggregate clause is deprecated. Use select/groupBy for all metrics/analytics. Only use aggregate for vector similarity until select supports it.
 */
export type AggregateCollectFn = {
  alias: string
  field?: string
  fn: 'collect'
  limit?: number
  orderBy?: Order
  skip?: number
  unique?: boolean
}

/**
 * @deprecated See AggregateCollectFn.
 */
export type AggregateCollectNestedFn = Omit<AggregateCollectFn, 'field'> & {
  aggregate?: { [field: string]: AggregateCollectNestedFn }
}

/**
 * @deprecated See AggregateCollectFn.
 */
export type AggregateCountFn = { field?: string; fn: 'count'; unique?: boolean; alias?: string }

/**
 * @deprecated See AggregateCollectFn.
 */
export type AggregateTimeBucketFn = {
  field: string
  fn: 'timeBucket'
  alias?: string
  granularity: 'day' | 'week' | 'month' | 'quarter' | 'year' | 'months'
  // When granularity === 'months', size (>0) defines the number of months per bucket (e.g. 2, 3, 6, 12)
  size?: number
}

// eslint-disable-next-line @typescript-eslint/no-unused-vars
/**
 * @deprecated The aggregate clause is deprecated. Use select/groupBy for all metrics/analytics. Only use aggregate for vector similarity until select supports it.
 */
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

/**
 * @deprecated See AggregateCollectFn.
 */
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

// ── Select Expression System ───────────────────────────────────────────────

export type CollectExpr = {
  from: string
  select?: SelectExprMap
  orderBy?: Order
  limit?: number
  skip?: number
  unique?: boolean
}

export type TimeBucketExpr = {
  /** Field reference, e.g. "$record.createdAt" */
  field: string
  unit:
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
  /** Required when unit is months/hours/minutes/seconds/years */
  size?: number
}

export type Expr =
  | string // "$record.field" field ref or "$alias" alias ref
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

type InferExprType<E extends Expr> =
  E extends string ? any
  : E extends number ? number
  : E extends boolean ? boolean
  : E extends { $sum: any } ? number
  : E extends { $avg: any } ? number
  : E extends { $count: any } ? number
  : E extends { $min: any } ? number
  : E extends { $max: any } ? number
  : E extends { $divide: any } ? number
  : E extends { $multiply: any } ? number
  : E extends { $add: any } ? number
  : E extends { $subtract: any } ? number
  : E extends { $collect: any } ? Array<any>
  : E extends { $timeBucket: any } ? string
  : any

export type ExtractSelectFields<S extends SelectExprMap> = {
  [K in keyof S]: InferExprType<S[K]>
}

export type SelectClause = {
  select?: SelectExprMap
}

export type SearchQuery<S extends Schema = any> = SearchQueryLabelsClause &
  PaginationClause &
  OrderClause<S> &
  WhereClause<S> &
  SelectClause &
  AggregateClause &
  GroupByClause

export type RelationshipEndpointQuery<S extends Schema = any> = SearchQueryLabelsClause & WhereClause<S>

export type RelationshipSearchQuery<S extends Schema = any> = PaginationClause &
  OrderClause<any> & {
    /**
     * Relationship-edge predicates. `type` maps to the relationship type and every other
     * field maps to a user-defined property stored on the relationship.
     */
    where?: Where<S> & { type?: any; direction?: 'in' | 'out' }
    source?: RelationshipEndpointQuery
    target?: RelationshipEndpointQuery
  }

/** Redeclare Models type in order to have suggestions over related records fields **/
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface Models {}
