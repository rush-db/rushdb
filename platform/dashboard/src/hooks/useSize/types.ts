import type { RefCallback, RefObject } from 'react'

export type TSize = {
  height: number
  width: number
}

export type TMeasurableElement = HTMLElement | SVGElement | null

export type TUseSizeResult<T extends TMeasurableElement> = {
  /** get access to internal element ref */
  elementRef: RefObject<T>
  /** attach this to a measurable element */
  ref: RefCallback<T>
} & TSize
