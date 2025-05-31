import { useEffect, useState, useRef } from 'react'

export function useWaitForSelectorStable(selector: string, stableMs: number = 100): boolean {
  const [ready, setReady] = useState(false)
  const timerRef = useRef<NodeJS.Timeout | null>(null)

  useEffect(() => {
    if (!selector) {
      setReady(false)
      return
    }

    let observer: MutationObserver | null = null

    const checkNode = () => {
      return document.querySelector(selector) !== null
    }

    const startTimer = () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current)
      }

      timerRef.current = setTimeout(() => {
        if (checkNode()) {
          setReady(true)
        } else {
          setReady(false)
          startObserving()
        }
      }, stableMs)
    }

    const startObserving = () => {
      if (checkNode()) {
        startTimer()
        return
      }

      observer = new MutationObserver((mutations) => {
        if (checkNode()) {
          if (observer) {
            observer.disconnect()
          }

          startTimer()
        }
      })

      observer.observe(document.documentElement, {
        subtree: true,
        childList: true
      })
    }

    setReady(false)

    if (timerRef.current) {
      clearTimeout(timerRef.current)
      timerRef.current = null
    }

    if (observer) {
      const currentObserver = observer as MutationObserver
      currentObserver.disconnect()
      observer = null
    }

    startObserving()

    return () => {
      if (observer) {
        observer.disconnect()
      }
      if (timerRef.current) {
        clearTimeout(timerRef.current)
      }
    }
  }, [selector, stableMs])

  return ready
}
