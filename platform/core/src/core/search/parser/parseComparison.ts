// only Record property (key) + criteria (input)
import { arrayIsConsistent } from '@/common/utils/arrayIsConsistent'
import { containsAllowedKeys } from '@/common/utils/containsAllowedKeys'
import { isArray } from '@/common/utils/isArray'
import { isObject } from '@/common/utils/isObject'
import { isPrimitive } from '@/common/utils/isPrimitive'
import { toBoolean } from '@/common/utils/toBolean'
import { RUSHDB_KEY_PROPERTIES_META, RUSHDB_VALUE_NULL, ISO_8601_REGEX } from '@/core/common/constants'
import { PropertyExpression, VectorExpression } from '@/core/common/types'
import { DatetimeObject } from '@/core/property/property.types'
import { QueryCriteriaParsingError } from '@/core/search/parser/errors'
import {
  COMPARISON_OPERATORS_MAP,
  comparisonOperators,
  datetimeOperators,
  vectorOperators
} from '@/core/search/search.constants'
import { TSearchQueryBuilderOptions } from '@/core/search/search.types'

const formatCriteriaValue = (value: unknown): string => {
  if (typeof value === 'string') {
    return `"${value}"`
  } else if (value === null) {
    return `"${RUSHDB_VALUE_NULL}"`
  } else {
    return `${value}`
  }
}

const formatField = (field: string, options: TSearchQueryBuilderOptions) => {
  return options.nodeAlias ? `${options.nodeAlias}.${field}` : field
}

const vectorConditionQueryPrefix = (field: string, options: TSearchQueryBuilderOptions) => {
  return `apoc.convert.fromJsonMap(\`${options.nodeAlias}\`.\`${RUSHDB_KEY_PROPERTIES_META}\`).\`${field}\` = "vector"`
}

const formatVectorForQuery = (
  value: VectorExpression['$vector'],
  field: string,
  options: TSearchQueryBuilderOptions
) => {
  if (isPrimitive(value.query)) {
    return `gds.similarity.${value.fn}(\`${options.nodeAlias}\`.\`${field}\`, ${value.value}) = ${value.query}`
  }
}

const datetimeConditionQueryPrefix = (field: string, options: TSearchQueryBuilderOptions) => {
  return `apoc.convert.fromJsonMap(\`${options.nodeAlias}\`.\`${RUSHDB_KEY_PROPERTIES_META}\`).\`${field}\` = "datetime"`
}

const formatDateTimeForQuery = (value: string | DatetimeObject) => {
  if (typeof value === 'string' && ISO_8601_REGEX.test(value)) {
    return `datetime("${value}")`
  } else if (isObject(value) && '$year' in value) {
    const datetimeParts = []

    // Dynamically add only the parts that are present
    const parts = [
      'year',
      'month',
      'day',
      'hour',
      'minute',
      'second',
      'millisecond',
      'microsecond',
      'nanosecond'
    ]
    parts.forEach((part) => {
      const key = `$${part}`
      if (key in value) {
        datetimeParts.push(`${part}: ${value[key]}`)
      }
    })

    return `datetime({${datetimeParts.join(', ')}})`
  }

  return undefined
}

export const parseComparison = (
  key: string,
  input: PropertyExpression,
  options?: TSearchQueryBuilderOptions
) => {
  const field = formatField(key, options)
  const datetimeQueryPrefix = datetimeConditionQueryPrefix(key, options)

  if (isPrimitive(input)) {
    return `any(value IN ${field} WHERE value = ${formatCriteriaValue(input)})`
  } else if (isObject(input)) {
    // DATE
    if (toBoolean(input) && containsAllowedKeys(input, datetimeOperators)) {
      const datetimeCriteria = formatDateTimeForQuery(input as DatetimeObject)
      return `any(value IN ${field} WHERE ${datetimeQueryPrefix} AND datetime(value) = ${datetimeCriteria})`
    }

    // VECTOR
    else if (toBoolean(input) && '$vector' in input) {
      return `(${vectorConditionQueryPrefix(key, options)} AND ${formatVectorForQuery(input?.$vector, key, options)})`
    }

    // COMPARISON
    else if (containsAllowedKeys(input, comparisonOperators)) {
      return Object.entries(input).map(([operator, value]) => {
        switch (operator) {
          case '$gt':
          case '$gte':
          case '$lt':
          case '$lte': {
            if (typeof value === 'number') {
              return `any(value IN ${field} WHERE value ${COMPARISON_OPERATORS_MAP[operator]} ${value})`
            } else if (
              toBoolean(value) &&
              (containsAllowedKeys(value, datetimeOperators) || typeof value === 'string')
            ) {
              const datetimeCriteria = formatDateTimeForQuery(value as DatetimeObject)
              if (!datetimeCriteria) {
                throw new QueryCriteriaParsingError(operator, value)
              }
              return `any(value IN ${field} WHERE ${datetimeQueryPrefix} AND datetime(value) ${COMPARISON_OPERATORS_MAP[operator]} ${datetimeCriteria})`
            } else {
              throw new QueryCriteriaParsingError(operator, value)
            }
          }
          case '$contains': {
            if (typeof value === 'string') {
              return `any(value IN ${field} WHERE value =~ "(?i).*${value}.*")`
            } else {
              throw new QueryCriteriaParsingError(operator, value)
            }
          }
          case '$startsWith':
          case '$endsWith': {
            const op = operator === '$endsWith' ? 'ENDS WITH' : 'STARTS WITH'
            if (typeof value === 'string') {
              return `any(value IN ${field} WHERE value ${op} "${value}")`
            } else {
              throw new QueryCriteriaParsingError(operator, value)
            }
          }
          case '$in':
          case '$nin': {
            const condition = () => {
              if (isArray(value) && toBoolean(value.length)) {
                if (arrayIsConsistent(value)) {
                  const criteria = value.map(formatCriteriaValue).join(', ')
                  return `value IN ${field} WHERE value IN [${criteria}]`
                } else if (value.every((v) => toBoolean(v) && containsAllowedKeys(v, datetimeOperators))) {
                  const datetimeQueryPrefix = datetimeConditionQueryPrefix(key, options)
                  const datetimeConditions = value.map((v) => formatDateTimeForQuery(v as DatetimeObject))
                  return `value IN [${datetimeConditions}] WHERE ${datetimeQueryPrefix} AND datetime(${field}) = value`
                } else {
                  throw new QueryCriteriaParsingError(operator, value)
                }
              } else {
                throw new QueryCriteriaParsingError(operator, value)
              }
            }

            return operator === '$nin' ? `none(${condition()})` : `any(${condition()})`
          }
          case '$ne': {
            if (isPrimitive(value)) {
              const criteria = formatCriteriaValue(value)
              return `any(value IN ${field} WHERE value <> ${criteria})`
            } else if (toBoolean(value) && containsAllowedKeys(value, datetimeOperators)) {
              const datetimeCriteria = formatDateTimeForQuery(value as DatetimeObject)

              return `any(value IN ${field} WHERE ${datetimeQueryPrefix} AND datetime(value) <> ${datetimeCriteria})`
            } else {
              throw new QueryCriteriaParsingError(operator, value)
            }
          }
        }
      })
    } else {
      throw new QueryCriteriaParsingError('unknown', input)
    }
  } else {
    throw new QueryCriteriaParsingError('unknown', input)
  }
}
