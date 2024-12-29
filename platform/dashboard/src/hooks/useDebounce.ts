import { useMemo } from 'react'

import type { AnyFunction } from '~/types'

import { debounce } from '~/lib/utils'

export const useDebounce = <Fn extends AnyFunction>(
  callback: Fn,
  delay: number
) => {
  return useMemo(() => debounce<Fn>(callback, delay), [delay, callback])
}
