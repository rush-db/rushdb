import { isObject } from '@/common/utils/isObject'
import { Where } from '@/core/common/types'
import { logicalOperators } from '@/core/search/search.constants'

const processLogicalOperator = (obj: Where, key: string): any => {
  const value = obj[key]
  if (isObject(value)) {
    return Object.keys(value).map((subKey) => {
      if (isObject(value[subKey])) {
        return { [subKey]: processCriteria(value[subKey]) }
      } else {
        return { [subKey]: value[subKey] }
      }
    })
  } else {
    return value
  }
}

const processNonLogicalOperator = (obj: Where, key: string): any => {
  if (isObject(obj[key])) {
    return processCriteria(obj[key])
  } else {
    return obj[key]
  }
}

export const processCriteria = (obj: Where): Where => {
  const result: Where = {}

  for (const key in obj) {
    if (logicalOperators.includes(key)) {
      result[key] = processLogicalOperator(obj, key)
    } else {
      result[key] = processNonLogicalOperator(obj, key)
    }
  }

  return result
}
