import type { RefCallback } from 'react'

import { useCallback, useMemo, useRef, useState } from 'react'

interface Args extends IntersectionObserverInit {
  freezeOnceVisible?: boolean
}

export function useIntersection<T extends HTMLElement>({
  threshold = 0,
  root = null,
  rootMargin = '0%',
  freezeOnceVisible = false
}: Args = {}) {
  const ref = useRef<T | null>(null)
  const [entry, setEntry] = useState<IntersectionObserverEntry>()

  const frozen = entry?.isIntersecting && freezeOnceVisible

  const updateEntry = ([entry]: IntersectionObserverEntry[]): void => {
    setEntry(entry)
  }

  const observer = useMemo(() => {
    const hasIOSupport = !!window.IntersectionObserver

    if (!hasIOSupport || frozen) return

    const observerParams = { threshold, root, rootMargin }
    return new IntersectionObserver(updateEntry, observerParams)
  }, [JSON.stringify(threshold), root, rootMargin, frozen])

  const setRef = useCallback(
    (node: T) => {
      if (ref.current) {
        // cleanup
        setEntry(undefined)
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

  return useMemo<{
    entry: IntersectionObserverEntry | undefined
    ref: RefCallback<T>
  }>(
    () => ({
      ref: setRef,
      entry
    }),
    [setRef, entry]
  )
}
