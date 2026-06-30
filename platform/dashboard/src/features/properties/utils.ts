import type { PropertyWithValue } from '@rushdb/javascript-sdk'

export const formatSinglePropertyValue = ({ value }: Pick<PropertyWithValue, 'type' | 'value'>) => {
  // null/undefined means the field is unset.
  if (value === null || typeof value === 'undefined') {
    return '—'
  }

  return value?.toString() ?? ''
}

export const formatPropertyValue = ({ value, type }: Pick<PropertyWithValue, 'type' | 'value'>): string => {
  if (Array.isArray(value)) {
    return value.map((v) => formatSinglePropertyValue({ value: v, type })).join(', ')
  }

  return formatSinglePropertyValue({ value, type })
}
