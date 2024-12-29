import { useCallback, useEffect, useRef } from 'react'

export const useFocusOutside = (
  handler: (event: FocusEvent) => void,
  { disabled }: { disabled?: boolean } = {}
) => {
  const focusCaptured = useRef<boolean>(false)

  const handleDocumentFocus = useCallback(
    (event: FocusEvent) => {
      if (!focusCaptured.current && handler) {
        handler(event)
      }
      focusCaptured.current = false
    },
    [handler]
  )

  const handleFocus = useCallback(() => {
    focusCaptured.current = true
  }, [])

  useEffect(() => {
    if (!disabled) {
      document.addEventListener('focusin', handleDocumentFocus)

      return () => {
        document.removeEventListener('focusin', handleDocumentFocus)
      }
    }
  }, [handleDocumentFocus, disabled])

  return {
    onFocus: disabled ? undefined : handleFocus
  } as const
}
