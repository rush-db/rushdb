import {
  comparisonOperators,
  datetimeOperators,
  logicalOperators,
  typeOperators
} from '@/core/search/search.constants'

export const allowedKeys = [
  ...comparisonOperators,
  ...datetimeOperators,
  ...logicalOperators,
  ...typeOperators
]

export const PROPERTY_WILDCARD_PROJECTION = '.*' as const
