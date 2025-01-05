import classNames from "classnames"
import { ComponentPropsWithoutRef, forwardRef, useEffect, useRef } from "react"
import { useInView } from "framer-motion"
import composeRefs from "~/utils/composeRefs"

export const LazyVideo = forwardRef<
  HTMLVideoElement,
  ComponentPropsWithoutRef<"video">
>(({ className, src, ...props }, ref) => {
  const videoRef = useRef<HTMLVideoElement>(null)

  const inView = useInView(videoRef)

  useEffect(() => {
    if (!videoRef.current) {
      return
    }

    if (inView) {
      videoRef.current.play()
    } else {
      videoRef.current.pause()
    }
  }, [inView])

  return (
    <video
      ref={composeRefs(ref, videoRef)}
      muted
      autoPlay={false}
      preload="none"
      playsInline
      className={classNames("relative w-full aspect-[16/10]", className)}
      {...props}
    >
      <source src={src} type="video/mp4" />
      Your browser does not support the video tag. Please try viewing this page
      in a modern browser.
    </video>
  )
})

LazyVideo.displayName = "LazyVideo"
