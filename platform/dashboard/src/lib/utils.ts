import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'
import { Toast, toast } from '~/elements/Toast'

import type { AnyFunction } from '~/types'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 *  Get element from array from a given step
 *
 * @example
 * ```tsx
 * getFromIndex(['peach', 'apple', 'orange'], 0, 1) // => 'apple'
 * getFromIndex(['peach', 'apple', 'orange'], 0, -1) // => 'orange'
 * ```
 */
export const getFromIndex = <T extends unknown[]>(
  array: T,
  currentIndex: number,
  step: number
): T[number] => {
  switch (true) {
    case step === 0:
      return array[currentIndex]
    case step < 0: {
      return array[(currentIndex + array.length + step) % array.length]
    }
    case step > 0: {
      return array[(currentIndex + step) % array.length]
    }
  }
}

export const addOrRemove = <T extends unknown[]>(
  array: T,
  item: T[number]
): T =>
  array.includes(item)
    ? (array.filter((i) => i !== item) as T)
    : ([...array, item] as T)

export function debounce<Fn extends AnyFunction>(
  callback: Fn,
  delayMs: number
) {
  let timeout: ReturnType<typeof setTimeout>

  return ((...args: Parameters<Fn>) => {
    if (timeout) clearTimeout(timeout)

    timeout = setTimeout(() => {
      const returnValue = callback(...args)

      return returnValue as ReturnType<Fn>
    }, delayMs)
  }) as (...args: Parameters<Fn>) => ReturnType<Fn>
}

/**
 * @description
 * Takes an Array<V>, and a grouping function,
 * and returns a Map of the array grouped by the grouping function.
 *
 * @param list An array of type V.
 * @param keyGetter A Function that takes the the Array type V as an input, and returns a value of type K.
 *                  K is generally intended to be a property key of V.
 *
 * @returns Map of the array grouped by the grouping function.
 */
export function groupBy<K, V>(
  list: Array<V>,
  keyGetter: (input: V) => K
): Map<K, Array<V>> {
  const map = new Map()
  list.forEach((item) => {
    const key = keyGetter(item)
    const collection = map.get(key)
    if (!collection) {
      map.set(key, [item])
    } else {
      collection.push(item)
    }
  })
  return map
}

export const removeDuplicates = <T extends unknown[]>(array: T) => {
  return [...new Set(array)] as T
}

export const composeEventHandlers =
  <E>(
    originalEventHandler?: (event: E) => void,
    ourEventHandler?: (event: E) => void,
    { checkForDefaultPrevented = true } = {}
  ) =>
  (event: E) => {
    originalEventHandler?.(event)

    if (
      checkForDefaultPrevented === false ||
      !(event as unknown as Event).defaultPrevented
    ) {
      return ourEventHandler?.(event)
    }
  }

export const normalizeString = (str: string) => str?.trim().toLowerCase()

export const range = (to: number): Array<number> => [...Array(to).keys()]

export function removeProperty<
  T extends {
    [key: string]: any
  },
  PropertyName extends keyof T
>(obj: T, property: PropertyName): Omit<T, PropertyName> {
  const copy = { ...obj }
  delete copy[property]
  return copy
}

export { clamp } from 'framer-motion'

export function isUrl(str: string) {
  const urlRegex =
    '^(?!mailto:)(?:(?:http|https|ftp)://)(?:\\S+(?::\\S*)?@)?(?:(?:(?:[1-9]\\d?|1\\d\\d|2[01]\\d|22[0-3])(?:\\.(?:1?\\d{1,2}|2[0-4]\\d|25[0-5])){2}(?:\\.(?:[0-9]\\d?|1\\d\\d|2[0-4]\\d|25[0-4]))|(?:(?:[a-z\\u00a1-\\uffff0-9]+-?)*[a-z\\u00a1-\\uffff0-9]+)(?:\\.(?:[a-z\\u00a1-\\uffff0-9]+-?)*[a-z\\u00a1-\\uffff0-9]+)*(?:\\.(?:[a-z\\u00a1-\\uffff]{2,})))|localhost)(?::\\d{2,5})?(?:(/|\\?|#)[^\\s]*)?$'
  const regex = new RegExp(urlRegex, 'i')
  return regex.test(str)
}

export const isInViewport = (
  target: HTMLElement,
  { completelyVisible = true }: { completelyVisible?: boolean } = {}
) => {
  const rect = target.getBoundingClientRect()
  const elemTop = rect.top
  const elemBottom = rect.bottom

  if (completelyVisible) {
    // Only completely visible elements return true:
    return elemTop >= 0 && elemBottom <= window.innerHeight
  } else {
    // Partially visible elements return true:
    return elemTop < window.innerHeight && elemBottom >= 0
  }
}

const isArray = Array.isArray
const keyList = Object.keys
const hasProp = Object.prototype.hasOwnProperty

export function isDeepEqual<T>(a: T, b: T): boolean {
  if (a === b) {
    return true
  }

  if (a && b && typeof a === 'object' && typeof b === 'object') {
    const arrA = isArray(a),
      arrB = isArray(b)
    let i, length, key

    if (arrA && arrB) {
      length = a.length
      if (length !== b.length) {
        return false
      }
      for (i = length; i-- !== 0; ) {
        if (!isDeepEqual(a[i], b[i])) {
          return false
        }
      }
      return true
    }

    if (arrA !== arrB) {
      return false
    }

    const dateA = a instanceof Date,
      dateB = b instanceof Date
    if (dateA !== dateB) {
      return false
    }
    if (dateA && dateB) {
      return a.getTime() === b.getTime()
    }

    const regexpA = a instanceof RegExp,
      regexpB = b instanceof RegExp
    if (regexpA !== regexpB) {
      return false
    }
    if (regexpA && regexpB) {
      return a.toString() === b.toString()
    }

    const keys = keyList(a)
    length = keys.length

    if (length !== keyList(b).length) {
      return false
    }

    for (i = length; i-- !== 0; ) {
      if (!hasProp.call(b, keys[i])) {
        return false
      }
    }

    for (i = length; i-- !== 0; ) {
      key = keys[i]
      if (!isDeepEqual(a[key as keyof typeof a], b[key as keyof typeof b])) {
        return false
      }
    }

    return true
  }

  return a !== a && b !== b
}

export const copyToClipboard = (
  text: string | number,
  {
    callback,
    showSuccessToast
  }: {
    callback?: () => void
    showSuccessToast?: boolean
  } = {}
) => {
  window.navigator.clipboard
    .writeText(String(text))
    .then(() => {
      if (showSuccessToast)
        toast({
          title: 'Copied to clipboard'
        })

      callback?.()
    })
    .catch(() =>
      toast({
        title: `Couldn't copy`,
        variant: 'danger'
      } as Toast)
    )
}

export const capitalize = (str: string) => {
  return str.charAt(0).toUpperCase() + str.slice(1)
}

export const getNumberOfLines = (value: string) =>
  value.split(/\r\n|\r|\n/).length
