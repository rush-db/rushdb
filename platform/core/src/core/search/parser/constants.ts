import {
  comparisonOperators,
  datetimeOperators,
  logicalOperators,
  vectorOperators
} from '@/core/search/search.constants'

export const allowedKeys = [
  ...comparisonOperators,
  ...datetimeOperators,
  ...logicalOperators,
  ...vectorOperators
]
