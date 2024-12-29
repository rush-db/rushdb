import { useCallback, useEffect, useMemo, useRef } from 'react'

export type PointerEventHandler = (
  event: MouseEvent | PointerEvent | TouchEvent
) => void

/**
 * A custom hook that runs provided callback
 * when user clicks outside of the element following react dom hierarchy.
 * Works with portals.
 *
 * Returns:
 * - bind object with events to attach to the element
 */
export const useClickOutside = (
  handler: PointerEventHandler,
  { disabled }: { disabled?: boolean } = { disabled: false }
) => {
  const clickCaptured = useRef<boolean>(false)

  const handleInnerClick = useCallback(() => {
    clickCaptured.current = true
  }, [])

  const handleDocumentClick = useCallback(
    (event: MouseEvent | PointerEvent | TouchEvent) => {
      if (!clickCaptured.current && handler) {
        handler(event)
      }
      clickCaptured.current = false
    },
    [handler]
  )

  useEffect(() => {
    if (!disabled) {
      document.addEventListener('pointerdown', handleDocumentClick)
    }
    return () => {
      document.removeEventListener('pointerdown', handleDocumentClick)
    }
  }, [handleDocumentClick])

  return useMemo(
    () => ({
      onPointerDown: disabled ? undefined : handleInnerClick
    }),
    [handleInnerClick]
  )
}
