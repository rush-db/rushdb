import { ISO_8601_REGEX } from '@/core/common/constants'
import {
  PROPERTY_TYPE_BOOLEAN,
  PROPERTY_TYPE_DATETIME,
  PROPERTY_TYPE_NULL,
  PROPERTY_TYPE_NUMBER,
  PROPERTY_TYPE_STRING
} from '@/core/property/property.constants'
import { TPropertySingleValue, TPropertyType } from '@/core/property/property.types'

export const suggestPropertyType = (value: TPropertySingleValue): TPropertyType => {
  if (typeof value === 'string') {
    return ISO_8601_REGEX.test(value) ? PROPERTY_TYPE_DATETIME : PROPERTY_TYPE_STRING
  } else if (typeof value === 'number') {
    return PROPERTY_TYPE_NUMBER
  } else if (typeof value === 'boolean') {
    return PROPERTY_TYPE_BOOLEAN
  } else if (value === null) {
    return PROPERTY_TYPE_NULL
  } else {
    return PROPERTY_TYPE_STRING
  }
}
