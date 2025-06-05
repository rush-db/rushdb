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

export function omit<T, K extends keyof T>(obj: T, keys: K[]): Omit<T, K> {
  const result = { ...obj }
  keys.forEach((key) => {
    delete result[key]
  })
  return result
}

export const getOwnProperties = <T>(input: T) => {
  if (isObject(input)) {
    return omit(input, ['__label', '__id', '__proptypes'] as unknown as (keyof T)[])
  }
  return input
}

export const removeUndefinedDeep = <T>(input: T): T => {
  if (isArray(input)) {
    return input.map(removeUndefinedDeep) as T
  } else if (isObject(input)) {
    return Object.fromEntries(
      Object.entries(input)
        // eslint-disable-next-line @typescript-eslint/no-unused-vars
        .filter(([_, v]) => v !== undefined)
        .map(([k, v]) => [k, removeUndefinedDeep(v)])
    ) as T
  }

  return input
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
