import classNames from "classnames"
import { RotateCcw } from "lucide-react"
import { ComponentPropsWithoutRef, useRef, useState } from "react"
import { IconButton } from "~/components/IconButton"
import { LazyVideo } from "~/sections/Dashboard/LazyVideo"

export function VideoBlock({
  className,
  src,
  ...props
}: ComponentPropsWithoutRef<"div"> &
  Pick<ComponentPropsWithoutRef<typeof LazyVideo>, "src">) {
  const videoRef = useRef<HTMLVideoElement>(null)

  const [ended, setEnded] = useState(false)

  const replay = () => {
    setEnded(false)
    if (!videoRef.current) {
      return
    }
    videoRef.current.currentTime = 0
    videoRef.current.play()
  }

  // const replayButton = (
  //   <IconButton
  //     size="large"
  //     variant="secondary"
  //     onClick={replay}
  //     aria-label="Replay"
  //   >
  //     <RotateCcw />
  //   </IconButton>
  // )

  return (
    <div
      className={classNames(
        "relative w-full aspect-[16/10] shadow-xl",
        className,
      )}
      {...props}
    >
      <LazyVideo
        className="animate-fadeIn"
        src={src}
        onEnded={() => replay()}
        ref={videoRef}
      />

      {/*{ended ? (*/}
      {/*  <div className="absolute inset-0 bg-fill2/70 grid place-items-center transition animate-fadeIn">*/}
      {/*    {replayButton}*/}
      {/*  </div>*/}
      {/*) : (*/}
      {/*  <div className="absolute bottom-5 right-5 bg-fill2/70">*/}
      {/*    {replayButton}*/}
      {/*  </div>*/}
      {/*)}*/}
    </div>
  )
}
