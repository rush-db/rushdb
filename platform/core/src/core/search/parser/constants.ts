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

export const PROPERTY_WILDCARD_PROJECTION = '.*' as const
