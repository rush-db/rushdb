import { uuidv7 } from 'uuidv7'

import { isArray } from '@/common/utils/isArray'
import { ISO_8601_REGEX } from '@/core/common/constants'
import { CreateEntityDtoSimple } from '@/core/entity/dto/create-entity.dto'
import { TImportOptions } from '@/core/entity/import-export/import.types'
import {
  PROPERTY_TYPE_BOOLEAN,
  PROPERTY_TYPE_DATETIME,
  PROPERTY_TYPE_NUMBER,
  PROPERTY_TYPE_STRING
} from '@/core/property/property.constants'
import {
  TPropertyPropertiesWithValue,
  TPropertySingleValue,
  TPropertyType,
  TPropertyValue
} from '@/core/property/property.types'

export const arrayIsConsistent = (arr: Array<unknown>): boolean =>
  arr.every((item) => typeof item === typeof arr[0])

export const getValueParameters = (value: TPropertyValue) => {
  if (Array.isArray(value)) {
    return {
      isEmptyArray: value.length === 0,
      isEmptyStringsArray: value.every((v: any) => v === ''),
      isInconsistentArray: !arrayIsConsistent(value)
    }
  } else {
    return { isEmptyString: value === '' }
  }
}

export const suggestPropertyType = (value: TPropertySingleValue): TPropertyType => {
  if (typeof value === PROPERTY_TYPE_STRING) {
    return ISO_8601_REGEX.test(value as string) ? PROPERTY_TYPE_DATETIME : PROPERTY_TYPE_STRING
  } else if (typeof value === PROPERTY_TYPE_NUMBER) {
    return PROPERTY_TYPE_NUMBER
  } else if (typeof value === PROPERTY_TYPE_BOOLEAN) {
    return PROPERTY_TYPE_BOOLEAN
  } else {
    return PROPERTY_TYPE_STRING
  }
}

// null/undefined means "field unset" — such values are dropped on write, never stored.
const isNullish = (value: unknown): boolean => value === null || value === undefined

const processArrayValue = (value: any[], options: Omit<TImportOptions, 'returnResult'>) => {
  const { isEmptyArray, isInconsistentArray } = getValueParameters(value)
  if (isEmptyArray) {
    return { type: PROPERTY_TYPE_STRING, value: [] }
  }
  if (isInconsistentArray || !options.suggestTypes) {
    return { type: PROPERTY_TYPE_STRING, value: value.map(String) }
  }

  return { type: suggestPropertyType(value[0]), value }
}

const processNonArrayValue = (value: TPropertySingleValue, options: Omit<TImportOptions, 'returnResult'>) => {
  if (!options.suggestTypes) {
    return { type: PROPERTY_TYPE_STRING, value: String(value) }
  }
  return { type: suggestPropertyType(value), value }
}

const buildDefaultOptions = (options: Omit<TImportOptions, 'returnResult'> = {}) => ({
  ...options,
  suggestTypes: options.suggestTypes ?? true
})

export const prepareProperties = (
  data: Record<string, TPropertyValue>,
  options: Omit<TImportOptions, 'returnResult'> = {}
) => {
  const defaultOptions = buildDefaultOptions(options)
  const skipEmpty = defaultOptions.skipEmptyValues === true

  return Object.entries(data).flatMap(([name, value]): TPropertyPropertiesWithValue[] => {
    // null/undefined scalar → field is unset, drop it entirely
    if (isNullish(value)) {
      return []
    }
    // skipEmptyValues: an empty string is treated as unset (mirrors null). 0 and false are real values.
    if (skipEmpty && value === '') {
      return []
    }

    if (isArray(value)) {
      const hadElements = value.length > 0
      // always strip null/undefined; with skipEmptyValues, '' elements are stripped too
      const cleaned = value.filter((item) => !isNullish(item) && !(skipEmpty && item === ''))
      // an array reduced to nothing collapses to "unset"; a genuinely empty [] is preserved unless
      // skipEmptyValues is set, in which case it is also dropped
      if ((hadElements || skipEmpty) && cleaned.length === 0) {
        return []
      }
      const { type, value: processedValue } = processArrayValue(cleaned, defaultOptions)
      return [{ name, type, value: processedValue, id: uuidv7() } as TPropertyPropertiesWithValue]
    }

    const { type, value: processedValue } = processNonArrayValue(value, defaultOptions)
    return [{ name, type, value: processedValue, id: uuidv7() } as TPropertyPropertiesWithValue]
  })
}

export const normalizeRecord = ({
  label,
  options = {},
  data
}: CreateEntityDtoSimple & {
  parentId?: string
}) => ({
  label,
  properties: prepareProperties(data, buildDefaultOptions(options))
})
