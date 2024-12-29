import { isArray } from '@/common/utils/isArray'
import { isPrimitive } from '@/common/utils/isPrimitive'
import { isPrimitiveArray } from '@/common/utils/isPrimitiveArray'

const isPrimitiveRushDBValue = (value: any) => {
  if (isArray(value)) {
    return isPrimitiveArray(value)
  }

  return isPrimitive(value)
}

export const pickPrimitives = (input: any) => {
  const result = {}

  Object.entries(input).forEach(([key, value]) => {
    if (isPrimitiveRushDBValue(value)) {
      result[key] = value
    }
  })

  return result
}
