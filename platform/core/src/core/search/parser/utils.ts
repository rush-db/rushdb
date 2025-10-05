import { containsAllowedKeys } from '@/common/utils/containsAllowedKeys'
import { isArray } from '@/common/utils/isArray'
import { isEmptyObject } from '@/common/utils/isEmptyObject'
import { isObject } from '@/common/utils/isObject'
import { isPrimitive } from '@/common/utils/isPrimitive'
import { RUSHDB_KEY_PROPERTIES_META } from '@/core/common/constants'
import { Where, MaybeArray, TVectorSearchFn, Aggregate } from '@/core/common/types'
import { allowedKeys } from '@/core/search/parser/constants'
import { TSearchQueryBuilderOptions } from '@/core/search/search.types'

import { RELATION_CLAUSE_OPERATOR, ALIAS_CLAUSE_OPERATOR, ID_CLAUSE_OPERATOR } from '../search.constants'

export const wrapInParentheses = (input: string) => `(${input})`

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
      if (isObject(value) && isSubQuery(value as Where) && key !== '$vector') {
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
      if (key === '$vector' && isObject(value)) {
        return 'fn' in value && 'threshold' in value && 'query' in value
      } else {
        return allowedKeys.includes(key) && isPropertyCriteria(value as Where)
      }
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

export const vectorConditionQueryPrefix = (field: string, options: TSearchQueryBuilderOptions) => {
  return `${options.nodeAlias}.\`${field}\` IS NOT NULL AND apoc.convert.fromJsonMap(${options.nodeAlias}.\`${RUSHDB_KEY_PROPERTIES_META}\`).\`${field}\` = "vector"`
}

export function safeGdsSimilarity(
  method: `gds.similarity.${TVectorSearchFn}`,
  recordAlias: string,
  field: string,
  query: string,
  asPart: string
) {
  const nodeVec = `\`${recordAlias}\`.\`${field}\``
  const queryVec = `[${query}]`
  return `CASE
    WHEN ${vectorConditionQueryPrefix(field, { nodeAlias: recordAlias })} AND size(${nodeVec}) = size(${queryVec})
    THEN ${method}(${nodeVec}, ${queryVec})
    ELSE null
  END AS ${asPart}`
}
