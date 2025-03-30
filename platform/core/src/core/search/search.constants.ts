// Sort directions
export const SORT_ASC = 'asc' as const
export const SORT_DESC = 'desc' as const

export const COMPARISON_OPERATORS_MAP = {
  $gt: '>',
  $gte: '>=',
  $lt: '<',
  $lte: '<=',
  $ne: '<>'
} as const

export const comparisonOperators = [
  '$in',
  '$ne',
  '$nin',
  '$gte',
  '$gt',
  '$lte',
  '$lt',
  '$contains',
  '$startsWith',
  '$endsWith'
  // '$vector'
]

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
export const vectorOperators = ['$vector']

export const logicalOperators = ['$and', '$or', '$xor', '$nor', '$not']
export const RELATION_CLAUSE_OPERATOR = '$relation'
export const ALIAS_CLAUSE_OPERATOR = '$alias'
export const ID_CLAUSE_OPERATOR = '$id'
