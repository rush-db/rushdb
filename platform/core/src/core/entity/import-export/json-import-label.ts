import { isArray } from '@/common/utils/isArray'
import { isObject } from '@/common/utils/isObject'
import { isPrimitiveArray } from '@/common/utils/isPrimitiveArray'

function isComplexImportValue(value: unknown): boolean {
  return isObject(value) || (isArray(value) && !isPrimitiveArray(value))
}

export function jsonImportRequiresLabel(data: unknown): boolean {
  if (isArray(data)) {
    return true
  }

  if (!isObject(data)) {
    return true
  }

  const values = Object.values(data)

  if (values.length === 0) {
    return true
  }

  return values.some((value) => !isComplexImportValue(value))
}
