import {
  comparisonOperators,
  datetimeOperators,
  logicalOperators,
  typeOperators,
  vectorOperators
} from '@/core/search/search.constants'

export const allowedKeys = [
  ...comparisonOperators,
  ...datetimeOperators,
  ...logicalOperators,
  ...typeOperators,
  ...vectorOperators
]
