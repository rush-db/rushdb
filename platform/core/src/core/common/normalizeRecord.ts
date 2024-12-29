import { uuidv7 } from 'uuidv7'

import { isArray } from '@/common/utils/isArray'
import { ISO_8601_REGEX } from '@/core/common/constants'
import { TPropertyPropertiesNormalized } from '@/core/property/model/property.interface'
import {
  PROPERTY_TYPE_BOOLEAN,
  PROPERTY_TYPE_DATETIME,
  PROPERTY_TYPE_NULL,
  PROPERTY_TYPE_NUMBER,
  PROPERTY_TYPE_STRING
} from '@/core/property/property.constants'
import { TPropertyType, TPropertyValue } from '@/core/property/property.types'

export const arrayIsConsistent = (arr: Array<unknown>): boolean =>
  arr.every((item) => typeof item === typeof arr[0])

export const getValueParameters = (value: TPropertyValue) => {
  if (Array.isArray(value)) {
    return {
      isEmptyArray: value.length === 0,
      isEmptyStringsArray: value.every((v) => v === ''),
      isInconsistentArray: !arrayIsConsistent(value)
    }
  } else {
    return { isEmptyString: value === '' }
  }
}

export const suggestPropertyType = (value: TPropertyValue): TPropertyType => {
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

const processArrayValue = (value: any[], suggestTypes: boolean) => {
  const { isEmptyArray, isInconsistentArray } = getValueParameters(value)
  if (isEmptyArray) {
    return { type: PROPERTY_TYPE_STRING, value: [] }
  }
  if (isInconsistentArray || !suggestTypes) {
    return { type: PROPERTY_TYPE_STRING, value: value.map(String) }
  }
  return { type: suggestPropertyType(value[0]), value }
}

const processNonArrayValue = (value: TPropertyType, suggestTypes: boolean) => {
  if (!suggestTypes) {
    return { type: PROPERTY_TYPE_STRING, value: String(value) }
  }
  const type = suggestPropertyType(value)
  return { type, value: type === PROPERTY_TYPE_NULL ? null : value }
}

export const prepareProperties = (
  data: Record<string, TPropertyValue>,
  options: { suggestTypes: boolean } = { suggestTypes: true }
) =>
  Object.entries(data).map(([name, value]) => {
    const { type, value: processedValue } = isArray(value)
      ? processArrayValue(value, options.suggestTypes)
      : // eslint-disable-next-line @typescript-eslint/ban-ts-comment
        // @ts-ignore
        processNonArrayValue(value, options.suggestTypes)

    return { name, type, value: processedValue, id: uuidv7() }
  }) as TPropertyPropertiesNormalized[]

export const normalizeRecord = ({
  label,
  options = { suggestTypes: true },
  payload
}: {
  label?: string
  options?: { suggestTypes: boolean }
  parentId?: string
  payload: Record<string, TPropertyValue>
}) => ({
  label,
  properties: prepareProperties(payload, options)
})
