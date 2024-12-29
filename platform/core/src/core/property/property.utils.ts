import { uuidv7 } from 'uuidv7'

import { isArray } from '@/common/utils/isArray'
import { RUSHDB_VALUE_EMPTY_ARRAY, RUSHDB_VALUE_NULL } from '@/core/common/constants'
import {
  PROPERTY_TYPE_BOOLEAN,
  PROPERTY_TYPE_DATETIME,
  PROPERTY_TYPE_NULL,
  PROPERTY_TYPE_NUMBER,
  PROPERTY_TYPE_STRING
} from '@/core/property/property.constants'
import { TPropertySingleValue, TPropertyType } from '@/core/property/property.types'

import { PropertyDto } from './dto/property.dto'

export const splitValueBySeparator = (value: TPropertySingleValue, separator: string): string[] =>
  String(value)
    .trim()
    .split(separator)
    .map((string) => string.trim())
    .filter((value) => value !== '')

export const normalizeProperties = (arr: Array<PropertyDto>): Array<PropertyDto> => {
  return arr.reduce((acc, prop) => {
    const matchedIndex = acc.findIndex((accumulatedProp) => {
      const hasNameMatch = accumulatedProp?.name?.trim() === prop?.name?.trim()
      const hasTypeMatch = accumulatedProp?.type === prop.type
      return hasNameMatch && hasTypeMatch
    })
    const matched = matchedIndex >= 0
    const normalizedValue = prepareValues(prop)

    const { valueSeparator, ...rest } = prop

    if (matched) {
      acc[matchedIndex] = {
        ...acc[matchedIndex],
        id: uuidv7(),
        value: [].concat(acc[matchedIndex].value, normalizedValue)
      }
    } else {
      acc.push({
        ...rest,
        id: uuidv7(),
        value: normalizedValue
      })
    }

    return acc
  }, [])
}

export function removeUndefinedKeys(obj) {
  for (const propName in obj) {
    if (obj[propName] === null || obj[propName] === undefined) {
      delete obj[propName]
    }
  }
  return obj
}

const parseValue = (value: TPropertySingleValue, type: TPropertyType) => {
  switch (type) {
    case PROPERTY_TYPE_NUMBER: {
      return Number(value)
    }
    case PROPERTY_TYPE_BOOLEAN: {
      return Boolean(value)
    }
    case PROPERTY_TYPE_NULL: {
      return RUSHDB_VALUE_NULL
    }
    case PROPERTY_TYPE_DATETIME:
    case PROPERTY_TYPE_STRING:
    default: {
      return String(value)
    }
  }
}

const prepareValues = ({ value, valueSeparator, type }: PropertyDto) => {
  if (isArray(value)) {
    return value.length ? value.map((v) => parseValue(v, type)) : RUSHDB_VALUE_EMPTY_ARRAY
  } else if (valueSeparator && typeof valueSeparator === 'string') {
    const valuesRaw = splitValueBySeparator(value, valueSeparator)
    return valuesRaw.map((v) => parseValue(v, type))
  } else {
    return parseValue(value, type)
  }
}
