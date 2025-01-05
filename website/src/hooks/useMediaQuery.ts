import { useEffect, useState } from "react"

/**
 * useMediaQuery Hook
 *
 * Returns true if the document matches the media query, false otherwise.
 *
 * @param query - The media query string.
 * @returns A boolean indicating whether the document matches the media query.
 */
export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState<boolean>(false)

  useEffect(() => {
    const mediaQueryList = window.matchMedia(query)

    setMatches(mediaQueryList.matches)

    const handleChange = (event: MediaQueryListEvent) => {
      setMatches(event.matches)
    }

    mediaQueryList.addEventListener("change", handleChange)

    return () => mediaQueryList.removeEventListener("change", handleChange)
  }, [query])

  return matches
}
