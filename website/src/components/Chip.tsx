import { ComponentPropsWithoutRef } from "react"
import cx from "classnames"

const chipVariants = {
  yellow: "border-accent-yellow text-accent-yellow",
  green: "border-accent-green text-accent-green",
  blue: "border-accent-blue text-accent-blue",
  red: "border-accent-red text-accent-red",
  purple: "border-accent-purple text-accent-purple",
  orange: "border-accent-orange text-accent-orange",
}

export const Chip = ({
  variant = "yellow",
  ...props
}: ComponentPropsWithoutRef<"h4"> & {
  variant: keyof typeof chipVariants
}) => (
  <h4
    className={cx(
      "rounded-full border px-2 uppercase text-xs font-mono",
      chipVariants[variant],
    )}
    {...props}
  />
)
