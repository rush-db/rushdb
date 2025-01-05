import { useEffect, useRef, useState } from "react"

type Size = {
  width: number
  height: number
}

export const useSize = <Element extends HTMLElement = HTMLDivElement>() => {
  const [size, setSize] = useState<Size>({
    width: 0,
    height: 0,
  })
  const ref = useRef<Element>(null)

  useEffect(() => {
    if (ref.current) {
      const resizeObserver = new ResizeObserver((entries) => {
        const entry = entries[0]
        const borderSizeEntry = entry["borderBoxSize"]
        const borderSize = Array.isArray(borderSizeEntry)
          ? borderSizeEntry[0]
          : borderSizeEntry

        setSize({
          width: borderSize["inlineSize"],
          height: borderSize["blockSize"],
        })
      })

      resizeObserver.observe(ref.current)

      return () => {
        resizeObserver.disconnect()
      }
    }
  }, [])

  return { width: size.width, height: size.height, ref }
}
