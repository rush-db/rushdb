import { arrayIsConsistent } from '@/common/utils/arrayIsConsistent'
import { containsAllowedKeys } from '@/common/utils/containsAllowedKeys'
import { isArray } from '@/common/utils/isArray'
import { isObject } from '@/common/utils/isObject'
import { isPrimitive } from '@/common/utils/isPrimitive'
import { toBoolean } from '@/common/utils/toBolean'
import { RUSHDB_KEY_PROPERTIES_META, ISO_8601_REGEX } from '@/core/common/constants'
import { PropertyExpression } from '@/core/common/types'
import { DatetimeObject } from '@/core/property/property.types'
import { QueryCriteriaParsingError } from '@/core/search/parser/errors'
import {
  COMPARISON_OPERATORS_MAP,
  comparisonOperators,
  datetimeOperators,
  typeOperators
} from '@/core/search/search.constants'
import { TSearchQueryBuilderOptions } from '@/core/search/search.types'

const formatCriteriaValue = (value: unknown): string => {
  if (typeof value === 'string') {
    const escaped = value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')
    return `"${escaped}"`
  } else {
    return `${value}`
  }
}

const formatField = (field: string, options: TSearchQueryBuilderOptions) => {
  return options.nodeAlias ? `${options.nodeAlias}.\`${field}\`` : `\`${field}\``
}

const datetimeConditionQueryPrefix = (field: string, options: TSearchQueryBuilderOptions) => {
  return `apoc.convert.fromJsonMap(${options.nodeAlias}.\`${RUSHDB_KEY_PROPERTIES_META}\`).\`${field}\` = "datetime"`
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

  // `null` means "field is unset/absent" (RushDB no longer stores null as a value).
  if (input === null) {
    return `${field} IS NULL`
  }

  if (isPrimitive(input)) {
    return `any(value IN ${field} WHERE value = ${formatCriteriaValue(input)})`
  } else if (isObject(input)) {
    // DATE
    if (toBoolean(input) && containsAllowedKeys(input, datetimeOperators)) {
      const datetimeCriteria = formatDateTimeForQuery(input as DatetimeObject)
      return `any(value IN ${field} WHERE ${datetimeQueryPrefix} AND datetime(value) = ${datetimeCriteria})`
    }

    // TYPE
    else if (containsAllowedKeys(input, typeOperators)) {
      return Object.entries(input).map(([operator, value]) => {
        switch (operator) {
          case '$type': {
            if (value === 'null') {
              // 'null' is no longer a stored type — treat it as an absence check.
              return `${field} IS NULL`
            } else if (typeof value === 'string') {
              return `apoc.convert.fromJsonMap(${options.nodeAlias}.\`${RUSHDB_KEY_PROPERTIES_META}\`).\`${key}\` = "${value}"`
            } else {
              throw new QueryCriteriaParsingError(operator, value)
            }
          }
        }
      })
    }

    // COMPARISON
    else if (containsAllowedKeys(input, comparisonOperators)) {
      return Object.entries(input).map(([operator, value]) => {
        switch (operator) {
          case '$eq': {
            if (value === null) {
              return `${field} IS NULL`
            } else if (isPrimitive(value)) {
              return `any(value IN ${field} WHERE value = ${formatCriteriaValue(value)})`
            } else if (toBoolean(value) && containsAllowedKeys(value, datetimeOperators)) {
              const datetimeCriteria = formatDateTimeForQuery(value as DatetimeObject)

              return `any(value IN ${field} WHERE ${datetimeQueryPrefix} AND datetime(value) = ${datetimeCriteria})`
            } else {
              throw new QueryCriteriaParsingError(operator, value)
            }
          }
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
            if (!isArray(value) || !toBoolean(value.length)) {
              throw new QueryCriteriaParsingError(operator, value)
            }
            // `null` in the set means "field absent"; split it from the value-membership check.
            const hasNull = value.some((v) => v === null)
            const nonNull = value.filter((v) => v !== null)

            const setCondition = () => {
              if (!nonNull.length) {
                return null
              }
              if (arrayIsConsistent(nonNull)) {
                const criteria = nonNull.map(formatCriteriaValue).join(', ')
                return `value IN ${field} WHERE value IN [${criteria}]`
              } else if (nonNull.every((v) => toBoolean(v) && containsAllowedKeys(v, datetimeOperators))) {
                const datetimeQueryPrefix = datetimeConditionQueryPrefix(key, options)
                const datetimeConditions = nonNull.map((v) => formatDateTimeForQuery(v as DatetimeObject))
                return `value IN [${datetimeConditions}] WHERE ${datetimeQueryPrefix} AND datetime(${field}) = value`
              } else {
                throw new QueryCriteriaParsingError(operator, value)
              }
            }

            const set = setCondition()

            if (operator === '$nin') {
              const parts: string[] = []
              if (set) {
                parts.push(`none(${set})`)
              }
              if (hasNull) {
                parts.push(`${field} IS NOT NULL`)
              }
              return parts.length > 1 ? `(${parts.join(' AND ')})` : parts[0]
            } else {
              const parts: string[] = []
              if (set) {
                parts.push(`any(${set})`)
              }
              if (hasNull) {
                parts.push(`${field} IS NULL`)
              }
              return parts.length > 1 ? `(${parts.join(' OR ')})` : parts[0]
            }
          }
          case '$ne': {
            if (value === null) {
              return `${field} IS NOT NULL`
            } else if (isPrimitive(value)) {
              const criteria = formatCriteriaValue(value)
              return `any(value IN ${field} WHERE value <> ${criteria})`
            } else if (toBoolean(value) && containsAllowedKeys(value, datetimeOperators)) {
              const datetimeCriteria = formatDateTimeForQuery(value as DatetimeObject)

              return `any(value IN ${field} WHERE ${datetimeQueryPrefix} AND datetime(value) <> ${datetimeCriteria})`
            } else {
              throw new QueryCriteriaParsingError(operator, value)
            }
          }
          case '$exists': {
            if (typeof value === 'boolean') {
              if (value) {
                // $exists: true - field must exist and not be null
                return `${field} IS NOT NULL`
              } else {
                // $exists: false - field must not exist or be null
                return `${field} IS NULL`
              }
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
