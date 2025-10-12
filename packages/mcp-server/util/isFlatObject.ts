import type { PropertyValue } from '@rushdb/javascript-sdk'

export const isArray = (item: any): item is Array<any> =>
  typeof item === 'object' && Array.isArray(item) && item !== null

export const isObject = (input: unknown): input is object =>
  input !== null && Object.prototype.toString.call(input) === '[object Object]'

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
