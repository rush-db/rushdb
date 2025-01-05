import React from "react"
import cx from "classnames"
import { variants, sizes as buttonSizes } from "~/components/Button"

const sizes = {
  small: cx(buttonSizes.small, "!w-9 !p-0"),
  medium: cx(buttonSizes.medium, "!w-11 !p-0"),
  large: cx(buttonSizes.large, "!w-11 !p-0"),
}

type Props = {
  variant: keyof typeof variants
  size?: keyof typeof sizes
}

export function IconButton<As extends React.ElementType = "button">({
  as,
  variant = "accent",
  size = "medium",
  className,
  ...props
}: TPolymorphicComponentProps<As> & Props) {
  const Element = as ?? "button"

  return (
    <Element
      className={cx(
        "grid place-items-center rounded-lg",
        variants[variant],
        sizes[size],
        className,
      )}
      {...props}
    />
  )
}
