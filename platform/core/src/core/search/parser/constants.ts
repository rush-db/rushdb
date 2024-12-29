import { comparisonOperators, datetimeOperators, logicalOperators } from '@/core/search/search.constants'

export const allowedKeys = [...comparisonOperators, ...datetimeOperators, ...logicalOperators]
