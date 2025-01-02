import type React from 'react'

import { useCallback, useEffect, useRef, useState } from 'react'

export type TSetStateFn<T> = (prevState?: T) => T

export type TUseControllableStateParams<T> = {
  defaultValue?: T
  onChange?: (prevState: T) => void
  value?: T
}

export function useControllableProp<T>(prop: T | undefined, internalState: T) {
  const isControlled = prop !== undefined
  const valueProp = isControlled && typeof prop !== 'undefined' ? prop : internalState

  return [isControlled, valueProp] as const
}

export function useControllableState<T>({
  value: valueProp,
  defaultValue,
  // eslint-disable-next-line @typescript-eslint/no-empty-function
  onChange
}: TUseControllableStateParams<T>) {
  const [uncontrolledState, setUncontrolledState] = useUncontrolledState({
    defaultValue,
    onChange
  })
  const isControlled = valueProp !== undefined
  const value = isControlled ? valueProp : uncontrolledState

  const setValue: React.Dispatch<React.SetStateAction<T | undefined>> = useCallback(
    (nextValue) => {
      if (isControlled) {
        const setter = nextValue as TSetStateFn<T>
        const value = typeof nextValue === 'function' ? setter(valueProp) : nextValue

        if (value !== valueProp && typeof onChange !== 'undefined') {
          onChange(value as T)
        }
      } else {
        setUncontrolledState(nextValue)
      }
    },
    [isControlled, valueProp, setUncontrolledState, onChange]
  )

  return [value, setValue] as [T, (value: T) => void]
}

function useUncontrolledState<T>({
  defaultValue,
  onChange
}: Omit<TUseControllableStateParams<T>, 'valueProp'>) {
  const uncontrolledState = useState<T | undefined>(defaultValue)
  const [value] = uncontrolledState
  const prevValueRef = useRef(value)

  useEffect(() => {
    if (prevValueRef.current !== value) {
      onChange?.(value as T)
      prevValueRef.current = value
    }
  }, [value, prevValueRef, onChange])

  return uncontrolledState
}
