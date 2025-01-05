import classNames from "classnames"
import { ComponentPropsWithoutRef } from "react"
import { GridItem } from "~/components/Grid"

export const RoundedGridItem = ({
  idx,
  className,
  firstOfFirstRow,
  firstOfLastRow,
  ...props
}: ComponentPropsWithoutRef<typeof GridItem>) => (
  <GridItem
    idx={idx}
    className={classNames(
      className,
      "rounded-lg first:rounded-tl-3xl last:rounded-br-3xl",
    )}
    lastOfFirstRow={classNames("rounded-tr-3xl", firstOfFirstRow)}
    firstOfLastRow={classNames("rounded-bl-3xl", firstOfLastRow)}
    {...props}
  />
)
