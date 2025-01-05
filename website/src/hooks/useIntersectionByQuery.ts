import { useEffect, useRef, useState } from "react"

export const useIntersectionByQuery = (querySelector: string) => {
  const observer = useRef<IntersectionObserver | null>(null)
  const [value, setValue] = useState(false)

  useEffect(() => {
    const handleObserver = (entries: IntersectionObserverEntry[]) => {
      setValue(entries.map((entry) => entry.isIntersecting).includes(true))
    }

    const elements = document.querySelectorAll(querySelector)

    observer.current = new IntersectionObserver(handleObserver, {
      rootMargin: "0% 0% -100% 0px",
    })

    elements.forEach((elem) => observer.current?.observe(elem))

    return () => {
      observer.current?.disconnect()
    }
  }, [querySelector])

  return value
}
