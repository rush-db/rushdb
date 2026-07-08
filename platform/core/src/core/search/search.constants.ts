import { toBoolean } from '@/common/utils/toBolean'
import { dbContextStorage } from '@/database/db-context'

// Sort directions
export const SORT_ASC = 'asc' as const
export const SORT_DESC = 'desc' as const

export const COMPARISON_OPERATORS_MAP = {
  $eq: '=',
  $gt: '>',
  $gte: '>=',
  $lt: '<',
  $lte: '<=',
  $ne: '<>'
} as const

export const comparisonOperators = [
  '$eq',
  '$in',
  '$ne',
  '$nin',
  '$gte',
  '$gt',
  '$lte',
  '$lt',
  '$contains',
  '$startsWith',
  '$endsWith',
  '$exists'
]

export const typeOperators = ['$type']

export const datetimeOperators = [
  '$day',
  '$hour',
  '$microsecond',
  '$millisecond',
  '$minute',
  '$month',
  '$nanosecond',
  '$second',
  '$year'
]
export const logicalOperators = ['$and', '$or', '$xor', '$nor', '$not']
export const RELATION_CLAUSE_OPERATOR = '$relation'
export const ALIAS_CLAUSE_OPERATOR = '$alias'
export const ID_CLAUSE_OPERATOR = '$id'
export const CYCLE_CLAUSE_OPERATOR = '$cycle'

export const DEFAULT_MAX_TRAVERSAL_HOPS = 10

/**
 * Effective hop ceiling for variable-length traversal (`$relation.hops`).
 * The cap only protects the shared default Neo4j: self-hosted deployments and
 * projects on a BYO external database may traverse unbounded — runaway queries
 * are still cut off by the transaction timeout (NEO4J_TRANSACTION_TIMEOUT_MS).
 */
export const resolveMaxTraversalHops = (): number => {
  if (toBoolean(process.env.RUSHDB_SELF_HOSTED) || dbContextStorage.getStore()?.externalConnection) {
    return Infinity
  }

  const configured = Number(process.env.RUSHDB_MAX_TRAVERSAL_HOPS)
  return Number.isInteger(configured) && configured > 0 ? configured : DEFAULT_MAX_TRAVERSAL_HOPS
}
