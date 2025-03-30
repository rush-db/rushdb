import { containsAllowedKeys } from '@/common/utils/containsAllowedKeys'
import { isArray } from '@/common/utils/isArray'
import { isEmptyObject } from '@/common/utils/isEmptyObject'
import { isObject } from '@/common/utils/isObject'
import { isPrimitive } from '@/common/utils/isPrimitive'
import { Where, MaybeArray, Condition } from '@/core/common/types'
import { allowedKeys } from '@/core/search/parser/constants'

import { RELATION_CLAUSE_OPERATOR, ALIAS_CLAUSE_OPERATOR, ID_CLAUSE_OPERATOR } from '../search.constants'

export const wrapInParentheses = (input: string) => `(${input})`

export const isSubQuery = (input: Where) => {
  if (!isObject(input)) {
    return false
  } else {
    return (
      // has `$id` in query object
      ID_CLAUSE_OPERATOR in input ||
      // has empty object `{}` in query object behind property
      isEmptyObject(input) ||
      // has `$relation` in query object behind property
      RELATION_CLAUSE_OPERATOR in input ||
      // has `$alias` in query object behind property
      ALIAS_CLAUSE_OPERATOR in input ||
      // Input object is current level criteria (like date-related operators or comparison)
      !isPropertyCriteria(input) ||
      !containsAllowedKeys(input, allowedKeys)
    )
  }
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
        return 'fn' in value && 'value' in value && 'query' in value
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
