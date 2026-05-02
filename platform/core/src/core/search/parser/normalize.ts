import { BadRequestException } from '@nestjs/common'

import { Aggregate, SelectExprMap } from '@/core/common/types'

/**
 * Validates mutual exclusivity of `select` and `aggregate` in a search query.
 *
 * - If `select` is present: returns it as a `SelectExprMap`.
 * - If only `aggregate` is present (no `select`): returns `null` — the caller
 *   should fall back to the legacy `buildAggregation` path.
 * - If both are present: throws a `BadRequestException`.
 * - If neither is present: returns `null`.
 */
export function normalizeSelectExpr(query: {
  select?: SelectExprMap
  aggregate?: Aggregate
}): SelectExprMap | null {
  const hasSelect = !!query.select && Object.keys(query.select).length > 0
  const hasAggregate = !!query.aggregate && Object.keys(query.aggregate).length > 0

  if (hasSelect && hasAggregate) {
    throw new BadRequestException(
      'Ambiguous query: "select" and "aggregate" cannot both be present. ' +
        'Use "select" (the canonical form) or "aggregate" (deprecated), not both.'
    )
  }

  if (hasSelect) {
    return query.select!
  }
  // Only aggregate present, or neither — return null so callers use legacy path.
  return null
}
