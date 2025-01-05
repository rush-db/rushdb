import classNames from "classnames"

import { ComponentPropsWithoutRef } from "react"

export function CodeWrapper({
  className,
  ...props
}: ComponentPropsWithoutRef<"div">) {
  return (
    <div
      className={classNames("flex flex-col gap-3 w-full", className)}
      {...props}
    />
  )
}
