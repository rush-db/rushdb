import { isArray } from '@/common/utils/isArray'
import { isObject } from '@/common/utils/isObject'

export const toBoolean = (value: any): boolean => {
  if (typeof value === 'string') {
    if (value.toLowerCase() === 'true') {
      return true
    } else if (value.toLowerCase() === 'false') {
      return false
    }
  }

  if (isArray(value) && value.length === 0) {
    return false
  }
  if (isObject(value) && Object.keys(value).length === 0) {
    return false
  }

  return Boolean(value)
}
