import { containsAllowedKeys } from '@/common/utils/containsAllowedKeys'
import { isArray } from '@/common/utils/isArray'
import { isEmptyObject } from '@/common/utils/isEmptyObject'
import { isObject } from '@/common/utils/isObject'
import { isPrimitive } from '@/common/utils/isPrimitive'
import { RUSHDB_VALUE_EMPTY_ARRAY } from '@/core/common/constants'
import { Where, MaybeArray, Aggregate } from '@/core/common/types'
import { allowedKeys } from '@/core/search/parser/constants'
import { buildSortCriteria } from '@/core/search/parser/orderBy'
import { TSearchSort } from '@/core/search/search.types'

import {
  RELATION_CLAUSE_OPERATOR,
  ALIAS_CLAUSE_OPERATOR,
  CYCLE_CLAUSE_OPERATOR,
  ID_CLAUSE_OPERATOR,
  SORT_ASC
} from '../search.constants'

export const wrapInParentheses = (input: string) => `(${input})`

// Deterministic across both parser passes (parseSubQuery and buildWhereClause walk
// the tree independently but increment ctx.level in lockstep).
export const traversalRelsVar = (level: number) => `rels${level}`

export const wrapInCurlyBraces = (input: string) => `{${input}}`

export const isSubQuery = (input: Where) => {
  if (!isObject(input)) {
    return false
  } else {
    return (
      // has `$id` in query object
      ID_CLAUSE_OPERATOR in input ||
      // has an empty object `{}` in a query object behind property
      isEmptyObject(input) ||
      // has `$relation` in a query object behind property
      RELATION_CLAUSE_OPERATOR in input ||
      // has `$alias` in a query object behind property
      ALIAS_CLAUSE_OPERATOR in input ||
      // has `$cycle` in a query object behind property
      CYCLE_CLAUSE_OPERATOR in input ||
      // Input object is current level criteria (like date-related operators or comparison)
      !isPropertyCriteria(input) ||
      !containsAllowedKeys(input, allowedKeys)
    )
  }
}

export const isNestedAggregate = (aggregate: Aggregate) => {
  const entries = Object.values(aggregate) as Array<any>
  return entries.some((instruction) => isObject(instruction?.aggregate))
}

export function splitCriteria(input: Where) {
  const currentLevel: Where = {}
  const subQueries: Where = {}

  // !containsAllowedKeys(value, allowedKeys)
  const split = (v: Where) =>
    Object.entries(v).forEach(([key, value]) => {
      if (isObject(value) && isSubQuery(value as Where)) {
        subQueries[key] = value
      } else {
        currentLevel[key] = value
      }
    })

  if (isArray(input)) {
    input.forEach(split)
  } else if (isObject(input)) {
    split(input)
  }

  return { currentLevel, subQueries }
}

export const isPropertyCriteria = (input: MaybeArray<Where<any>>) => {
  if (isObject(input)) {
    return Object.entries(input).every(([key, value]) => {
      return allowedKeys.includes(key) && isPropertyCriteria(value as Where)
    })
  } else if (isArray(input)) {
    return (input as Array<Where>).every(isPropertyCriteria)
  } else {
    return isPrimitive(input)
  }
}

export function isCurrentLevelCriteria(input: MaybeArray<Where>) {
  if (isObject(input)) {
    return Object.values(input).every(isPropertyCriteria)
  }

  if (isArray(input)) {
    return (input as Array<Where>).every(isCurrentLevelCriteria)
  }
}

export function nativeVectorSimilarity(
  method: 'vector.similarity.cosine' | 'vector.similarity.euclidean',
  recordAlias: string,
  field: string,
  query: string,
  asPart: string
) {
  const nodeVec = `\`${recordAlias}\`.\`${field}\``
  const queryVec = `[${query}]`
  return `CASE
    WHEN ${nodeVec} IS NOT NULL AND size(${nodeVec}) = size(${queryVec})
    THEN ${method}(${nodeVec}, ${queryVec})
    ELSE null
  END AS ${asPart}`
}

export function apocSortMapsArray(arrayClause: string, orderBy?: TSearchSort): string {
  const sortCriteria = buildSortCriteria(orderBy!)

  const orderByKeyPart = Object.entries(sortCriteria).map(([property, direction]) => {
    return `"${direction.toLowerCase() === SORT_ASC ? '^' : ''}${property}"`
  })[0]

  return `apoc.coll.sortMaps(${arrayClause}, ${orderByKeyPart})`
}

export function apocSortArray(arrayClause: string): string {
  return `apoc.coll.sort(apoc.coll.flatten(${arrayClause}))`
}

export function apocUniqArray(arrayClause: string): string {
  return `apoc.coll.toSet(${arrayClause})`
}

export function apocRemoveFromArray(arrayClause: string): string {
  return `apoc.coll.removeAll(${arrayClause}, ["${RUSHDB_VALUE_EMPTY_ARRAY}"])`
}
