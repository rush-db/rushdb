import { SearchDto } from '@/core/search/dto/search.dto'

/**
 * Returns true if the object looks like a relationship-traversal entry in a
 * `where` clause rather than a plain property-condition entry.
 *
 * A traversal value either:
 *  - contains `$relation` or `$alias` (explicitly identifies a graph edge), OR
 *  - contains at least one key that does NOT start with `$`
 *    (i.e. it carries property filters for the related entity)
 *
 * Plain property conditions look like `{ $contains: 'foo' }` — only `$`-keys.
 */
function isRelationTraversal(value: Record<string, unknown>): boolean {
  return Object.keys(value).some((k) => k === '$relation' || k === '$alias' || !k.startsWith('$'))
}

/**
 * Recursively walks a `where` clause and returns the maximum relationship-
 * traversal depth found.
 *
 * Depth increments every time a non-`$` key resolves to a relationship
 * traversal (see `isRelationTraversal`).  Logical operators ($and, $or, …)
 * recurse without incrementing depth.
 *
 * Examples:
 *   {}                                          → 0  (no traversal)
 *   { name: { $contains: 'a' } }               → 0  (property filter only)
 *   { Company: { name: 'Acme' } }              → 1  (one hop)
 *   { Company: { City: { name: 'NYC' } } }     → 2  (two hops — still OK)
 *   { Company: { City: { Country: {} } } }     → 3  (three hops — heavy)
 */
export function getWhereNestingDepth(where: unknown, currentDepth = 0): number {
  if (!where || typeof where !== 'object' || Array.isArray(where)) {
    return currentDepth
  }

  const obj = where as Record<string, unknown>
  let maxDepth = currentDepth

  for (const [key, value] of Object.entries(obj)) {
    if (value === null || value === undefined) {
      continue
    }

    if (key === '$and' || key === '$or' || key === '$not' || key === '$xor' || key === '$nor') {
      // Logical groupings — recurse without incrementing depth
      const items = Array.isArray(value) ? value : [value]
      for (const item of items) {
        maxDepth = Math.max(maxDepth, getWhereNestingDepth(item, currentDepth))
      }
    } else if (!key.startsWith('$') && value && typeof value === 'object' && !Array.isArray(value)) {
      const valueObj = value as Record<string, unknown>
      if (isRelationTraversal(valueObj)) {
        // Relationship hop — increase depth and recurse into the sub-query
        maxDepth = Math.max(maxDepth, getWhereNestingDepth(valueObj, currentDepth + 1))
      }
      // else: { $contains: 'foo' } style property condition — no depth change
    }
  }

  return maxDepth
}

/**
 * Returns true if the search query is considered "compute-heavy" for the
 * purpose of KU tracking on a shared (RushDB-managed) Neo4j instance.
 *
 * A search is heavy when it involves:
 *  - Any aggregation (`aggregate` map is non-empty), OR
 *  - A `groupBy` clause (implicit aggregation), OR
 *  - Relationship traversal depth > 2 in the `where` clause
 *    (depth ≤ 2 is considered normal and is not charged)
 *
 * This function must NOT be called for external-DB projects
 * (`project.customDb` is set) — callers are responsible for
 * that guard.
 */
export function isHeavySearch(
  searchQuery: Partial<Pick<SearchDto, 'select' | 'aggregate' | 'where' | 'groupBy'>>
): boolean {
  const hasAggregations =
    (!!searchQuery.select && Object.keys(searchQuery.select).length > 0) ||
    (!!searchQuery.aggregate && Object.keys(searchQuery.aggregate).length > 0)
  const hasGroupBy = Array.isArray(searchQuery.groupBy) && searchQuery.groupBy.length > 0
  const nestingDepth = getWhereNestingDepth(searchQuery.where)

  return hasAggregations || hasGroupBy || nestingDepth > 2
}
