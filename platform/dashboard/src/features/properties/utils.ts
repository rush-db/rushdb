import type { CollectPropertyWithValue } from '@collect.so/javascript-sdk'

export const formatSinglePropertyValue = ({
  type,
  value
}: Pick<CollectPropertyWithValue, 'type' | 'value'>) => {
  if (typeof value === 'undefined') {
    return '—'
  }
  // if (type === 'datetime') {
  //   return formatIsoToLocal(value as string)
  // }

  // @TODO: rework
  if (type === 'null') {
    return 'null'
  }

  return value?.toString() ?? ''
}

export const formatPropertyValue = ({
  value,
  type
}: Pick<CollectPropertyWithValue, 'type' | 'value'>): string => {
  if (Array.isArray(value)) {
    return value
      .map((v) => formatSinglePropertyValue({ value: v, type }))
      .join(', ')
  }

  return formatSinglePropertyValue({ value, type })
}
