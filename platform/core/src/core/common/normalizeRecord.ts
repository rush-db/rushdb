import { uuidv7 } from 'uuidv7'

import { isArray } from '@/common/utils/isArray'
import { ISO_8601_REGEX } from '@/core/common/constants'
import { CreateEntityDtoSimple } from '@/core/entity/dto/create-entity.dto'
import { TImportOptions } from '@/core/entity/import-export/import.types'
import {
  PROPERTY_TYPE_BOOLEAN,
  PROPERTY_TYPE_DATETIME,
  PROPERTY_TYPE_NULL,
  PROPERTY_TYPE_NUMBER,
  PROPERTY_TYPE_STRING,
  PROPERTY_TYPE_VECTOR
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
  } else if (value === null) {
    return PROPERTY_TYPE_NULL
  } else {
    return PROPERTY_TYPE_STRING
  }
}

const processArrayValue = (value: any[], options: Omit<TImportOptions, 'returnResult'>) => {
  const { isEmptyArray, isInconsistentArray } = getValueParameters(value)
  if (isEmptyArray) {
    return { type: PROPERTY_TYPE_STRING, value: [] }
  }
  if (isInconsistentArray || !options.suggestTypes) {
    return { type: PROPERTY_TYPE_STRING, value: value.map(String) }
  }

  if (
    options.suggestTypes &&
    options.castNumberArraysToVectors &&
    suggestPropertyType(value[0]) === PROPERTY_TYPE_NUMBER
  ) {
    return { type: PROPERTY_TYPE_VECTOR, value }
  }
  return { type: suggestPropertyType(value[0]), value }
}

const processNonArrayValue = (value: TPropertySingleValue, options: Omit<TImportOptions, 'returnResult'>) => {
  if (!options.suggestTypes) {
    return { type: PROPERTY_TYPE_STRING, value: String(value) }
  }
  const type = suggestPropertyType(value)
  return { type, value: type === PROPERTY_TYPE_NULL ? null : value }
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

  return Object.entries(data).map(([name, value]) => {
    const { type, value: processedValue } =
      isArray(value) ? processArrayValue(value, defaultOptions) : processNonArrayValue(value, defaultOptions)

    return { name, type, value: processedValue, id: uuidv7() }
  }) as TPropertyPropertiesWithValue[]
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
