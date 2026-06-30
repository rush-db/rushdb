import { uuidv7 } from 'uuidv7'

import { isArray } from '@/common/utils/isArray'
import {
  PROPERTY_TYPE_BOOLEAN,
  PROPERTY_TYPE_DATETIME,
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
    // A null/undefined value (or an array that is entirely null) means the field is unset — drop it.
    if (prop.value === null || prop.value === undefined) {
      return acc
    }
    if (
      isArray(prop.value) &&
      prop.value.length > 0 &&
      prop.value.every((v) => v === null || v === undefined)
    ) {
      return acc
    }

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
    case PROPERTY_TYPE_DATETIME:
    case PROPERTY_TYPE_STRING:
    default: {
      return String(value)
    }
  }
}

const isNullish = (value: unknown): boolean => value === null || value === undefined

const prepareValues = ({ value, valueSeparator, type }: PropertyDto) => {
  if (isArray(value)) {
    // null/undefined elements mean "unset" — strip them; a genuinely empty array is
    // preserved and stored as a real [] (no sentinel placeholder).
    const cleaned = value.filter((v) => !isNullish(v))
    return cleaned.length ? cleaned.map((v) => parseValue(v, type)) : []
  } else if (valueSeparator && typeof valueSeparator === 'string') {
    const valuesRaw = splitValueBySeparator(value, valueSeparator)
    return valuesRaw.map((v) => parseValue(v, type))
  } else {
    return parseValue(value, type)
  }
}
