import type { PropertyValue } from '../types/index.js'

export const isArray = (item: any): item is any[] =>
  typeof item === 'object' && Array.isArray(item) && item !== null

export const isObject = (input: unknown): input is object =>
  input !== null && Object.prototype.toString.call(input) === '[object Object]'

export const isEmptyObject = (input: unknown): boolean =>
  isObject(input) && Object.keys(input).length === 0

export const isObjectFlat = (input: any): input is Record<string, PropertyValue> => {
  return (
    isObject(input) &&
    Object.keys(input).every((key) => {
      const value = (input as Record<string, any>)[key]
      // Check if the value is an array
      if (isArray(value)) {
        // Check if every element in the array is of an allowed type (string, number, boolean, or null)
        return value.every(
          (element) =>
            typeof element === 'string' ||
            typeof element === 'number' ||
            typeof element === 'boolean' ||
            element === null
        )
      }
      // Check if the value is a non-null object (excluding arrays, which are also typeof 'object')
      return !(value && typeof value === 'object')
    })
  )
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
