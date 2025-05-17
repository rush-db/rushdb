import { containsAllowedKeys } from '@/common/utils/containsAllowedKeys'
import { isArray } from '@/common/utils/isArray'
import { isObject } from '@/common/utils/isObject'
import { isPrimitive } from '@/common/utils/isPrimitive'
import { isPrimitiveArray } from '@/common/utils/isPrimitiveArray'
import { toBoolean } from '@/common/utils/toBolean'
import { RUSHDB_KEY_ID } from '@/core/common/constants'
import { PropertyExpression, Where } from '@/core/common/types'
import { QueryCriteriaParsingError } from '@/core/search/parser/errors'
import { parseComparison } from '@/core/search/parser/parseComparison'
import { parseSubQuery } from '@/core/search/parser/parseSubQuery'
import { processLogicalGroupedCriteria } from '@/core/search/parser/processLogicalGroupedCriteria'
import { ParseContext } from '@/core/search/parser/types'
import { splitCriteria, wrapInParentheses } from '@/core/search/parser/utils'
import {
  comparisonOperators,
  datetimeOperators,
  ID_CLAUSE_OPERATOR,
  vectorOperators
} from '@/core/search/search.constants'
import { TSearchQueryBuilderOptions } from '@/core/search/search.types'

export const parseCurrentLevel = (
  key: string,
  input: Where,
  options?: TSearchQueryBuilderOptions,
  ctx?: ParseContext
) => {
  if (isPrimitive(input)) {
    const condition = parseComparison(key, input as PropertyExpression, options)
    return isArray(condition) ? condition.join(' AND ') : condition
  }

  if (isArray(input)) {
    const condition = input.map((v) => {
      return parseCurrentLevel(key, v, options, ctx)
    })

    // @TODO: Parenthesis ??? (...)
    return condition.flat().join(options.joinOperator ? ` ${options.joinOperator} ` : ' AND ')
  }

  if (isObject(input)) {
    const { currentLevel, subQueries } = splitCriteria(input)

    // SUB QUERY PROCESSING
    if (toBoolean(subQueries)) {
      Object.entries(subQueries).map(([k, value]) => {
        return parseSubQuery(k, value as Where, options, ctx)
      })
    } else {
      if (
        containsAllowedKeys(currentLevel, [...comparisonOperators, ...datetimeOperators, ...vectorOperators])
      ) {
        const condition = parseComparison(key, currentLevel as PropertyExpression, options)

        // @TODO: Parenthesis ??? (...)
        return isArray(condition) ? condition.join(' AND ') : condition
      }

      return Object.entries(currentLevel).map(([k, v]) => {
        switch (k) {
          case '$or': {
            let condition

            if (isArray(v) && isPrimitiveArray(v)) {
              condition = parseCurrentLevel(
                key,
                v,
                {
                  ...options,
                  joinOperator: 'OR'
                },
                ctx
              )
            } else {
              condition = processLogicalGroupedCriteria(
                key,
                v as Where,
                {
                  ...options,
                  joinOperator: 'OR'
                },
                ctx
              )
            }

            if (condition) {
              return wrapInParentheses(isArray(condition) ? condition.join(' OR ') : condition)
            }
            break
          }
          case '$xor': {
            let condition

            if (isArray(v) && isPrimitiveArray(v)) {
              condition = parseCurrentLevel(
                key,
                v,
                {
                  ...options,
                  joinOperator: 'XOR'
                },
                ctx
              )
            } else {
              condition = processLogicalGroupedCriteria(
                key,
                v as Where,
                {
                  ...options,
                  joinOperator: 'XOR'
                },
                ctx
              )
            }

            if (condition) {
              return wrapInParentheses(isArray(condition) ? condition.join(' XOR ') : condition)
            }
            break
          }
          case '$nor': {
            let condition

            if (isArray(v) && isPrimitiveArray(v)) {
              condition = parseCurrentLevel(
                key,
                v,
                {
                  ...options,
                  joinOperator: 'OR'
                },
                ctx
              )
            } else {
              condition = processLogicalGroupedCriteria(
                key,
                v as Where,
                {
                  ...options,
                  joinOperator: 'OR'
                },
                ctx
              )
            }

            if (condition) {
              return wrapInParentheses(
                `NOT${wrapInParentheses(isArray(condition) ? condition.join(' OR ') : condition)}`
              )
            }
            break
          }
          case '$not': {
            let condition

            if (isArray(v) && isPrimitiveArray(v)) {
              condition = parseCurrentLevel(key, v, options, ctx)
            } else {
              condition = processLogicalGroupedCriteria(key, v as Where, options, ctx)
            }
            if (condition) {
              return wrapInParentheses(
                `NOT${wrapInParentheses(isArray(condition) ? condition.join(' AND ') : condition)}`
              )
            }
            break
          }
          case '$and': {
            let condition

            if (isArray(v) && isPrimitiveArray(v)) {
              condition = parseCurrentLevel(
                key,
                v,
                {
                  ...options,
                  joinOperator: 'AND'
                },
                ctx
              )
            } else {
              condition = processLogicalGroupedCriteria(
                key,
                v as Where,
                {
                  ...options,
                  joinOperator: 'AND'
                },
                ctx
              )
            }
            if (condition) {
              return wrapInParentheses(isArray(condition) ? condition.join(' AND ') : condition)
            }
            break
          }
          default: {
            const keyToApply = k === ID_CLAUSE_OPERATOR ? RUSHDB_KEY_ID : k
            const condition = parseCurrentLevel(keyToApply, v as PropertyExpression, options, ctx)

            return wrapInParentheses(isArray(condition) ? condition.join(' AND ') : condition)
          }
        }
      })
    }
  }
  //
  else {
    throw new QueryCriteriaParsingError('unknown', input)
  }
}
