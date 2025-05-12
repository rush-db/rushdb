import type { PropertyValue } from '../types/index.js'

export const isArray = (item: any): item is Array<any> =>
  typeof item === 'object' && Array.isArray(item) && item !== null

export const isObject = (input: unknown): input is object =>
  input !== null && Object.prototype.toString.call(input) === '[object Object]'

export const isEmptyObject = (input: unknown): boolean => isObject(input) && Object.keys(input).length === 0

export const isPrimitive = (value: unknown) => {
  return (
    typeof value === 'number' || typeof value === 'string' || typeof value === 'boolean' || value === null
  )
}

export const isPropertyValue = (value: any): value is PropertyValue => {
  return isArray(value) ? value.every(isPrimitive) : isPrimitive(value)
}

export const isFlatObject = (input: any): input is Record<string, PropertyValue> => {
  return isObject(input) && Object.values(input).every(isPropertyValue)
}

export const isString = (input: any): input is string => typeof input === 'string'

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
