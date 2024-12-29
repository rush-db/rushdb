import { TAnyObject } from '@/common/types/utils'
import { isObject } from '@/common/utils/isObject'
import { uniqArray } from '@/common/utils/uniqArray'

export const collectValuesByKeysFromObject = (object: TAnyObject, keys: string[]) => {
  const res: string[] = []
  const getKeys = (object: TAnyObject) =>
    isObject(object) &&
    Object.entries(object).forEach(([key, value]: [string, unknown]) => {
      if (keys.includes(key)) {
        res.push(value as string)
      }

      // if (Array.isArray(value)) {
      //     value.forEach(getKeys);
      // }
      //
      // if (isObject(value)) {
      //     getKeys(value as TAnyObject);
      // }
    })
  getKeys(object)
  return uniqArray(res)
}
