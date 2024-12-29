import { isPrimitive } from '@/common/utils/isPrimitive'

export const arrayIsConsistent = (arr: Array<unknown>) => {
  if (arr.length === 0) {
    // @TODO: Should it be true in this case?
    return true
  }

  const firstElement = arr[0]

  return arr.every(
    (el) => isPrimitive(el) && (firstElement === null ? el === null : typeof el === typeof firstElement)
  )
}
