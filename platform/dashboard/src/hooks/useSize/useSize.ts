import type { RefObject } from 'react'

import { useCallback, useMemo, useRef, useState } from 'react'

import { useDebounce } from '~/hooks/useDebounce'

import type { TMeasurableElement, TSize, TUseSizeResult } from './types'

/**
 * A custom hook that measures element dimensions with resize observer
 *
 * returns:
 *  - ref: ref to attach to measurable element to
 *  - size: measured size
 */
export const useSize = <T extends TMeasurableElement = HTMLDivElement>({
  delay = 150,
  cb,
  disabled
}: {
  cb?: (value: { elementRef: RefObject<T>; size: TSize }) => void
  /* debounce size updates */
  delay?: number
  disabled?: boolean
} = {}): TUseSizeResult<T> => {
  const ref = useRef<T | null>(null)
  const [size, setSize] = useState<TSize>({ width: 0, height: 0 })
  const debouncedSetSize = useDebounce(setSize, delay)

  const observer = useMemo(
    () =>
      typeof window === 'undefined' ? null : (
        new ResizeObserver((entries) => {
          if (disabled) {
            return
          }

          if (!Array.isArray(entries) || !entries.length) {
            return
          }

          const [entry] = entries

          let width: number
          let height: number

          if (entry.borderBoxSize) {
            const borderSizeEntry = entry['borderBoxSize']
            const borderSize = Array.isArray(borderSizeEntry) ? borderSizeEntry[0] : borderSizeEntry
            width = borderSize['inlineSize']
            height = borderSize['blockSize']
          } else {
            width = entry.contentRect.width
            height = entry.contentRect.height
          }

          const size = {
            width,
            height
          }

          debouncedSetSize(size)

          if (cb) {
            cb({ size, elementRef: ref })
          }
        })
      ),
    [cb, debouncedSetSize, disabled]
  )

  const setRef = useCallback(
    (node: T) => {
      if (ref.current) {
        // cleanup
        setSize({ width: 0, height: 0 })
        observer?.unobserve(ref.current)
      }

      if (node) {
        // check if a node is actually passed. Otherwise node would be null
        observer?.observe(node)
      }

      // save a reference to the node
      ref.current = node
    },
    [observer]
  )

  return {
    width: size.width,
    height: size.height,
    ref: setRef,
    elementRef: ref
  } as TUseSizeResult<T>
}
