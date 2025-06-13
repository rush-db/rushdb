import React, { ComponentPropsWithoutRef } from 'react'
import cx from 'classnames'
import Link from 'next/link'

export const variants = {
  accent:
    'bg-accent text-accent-contrast hover:bg-accent-hover focus-visible:bg-accent-focus ring-accent-ring',
  outline: 'border hover:bg-secondary-hover hover:border-bg-secondary-hover ',
  primary: 'bg-primary ring-primary-ring text-primary-contrast hover:bg-primary-hover',
  secondary: 'bg-secondary text-secondary-content hover:bg-secondary-hover ring-secondary-ring',
  primaryText: 'hover:text-accent hover:bg-secondary-hover',
  accentText: 'text-accent hover:text-accent-hover',
  custom: ''
}

export const sizes = {
  small: 'h-9 gap-2 [&>svg]:w-[20px] [&>svg]:h-[20px] rounded-md px-3 font-medium',
  medium:
    'h-11 [&>svg]:w-[20px] [&>svg]:h-[20px] gap-2 rounded-lg py-2.5 px-3.5 md:py-2 md:px-3 md:text-sm text-base font-bold',
  large: 'h-11 [&>svg]:w-[20px] [&>svg]:h-[20px] rounded-lg'
}

type Props = {
  variant: keyof typeof variants
  size?: keyof typeof sizes
}

export function Button<As extends React.ElementType = 'button'>({
  as,
  variant = 'accent',
  size = 'medium',
  className,
  ...props
}: TPolymorphicComponentProps<As, Props>) {
  const Element = as ?? 'button'

  return (
    <Element
      className={cx('flex items-center justify-center transition', variants[variant], sizes[size], className)}
      {...props}
    />
  )
}

export const MainCta = ({
  children,
  href = 'https://app.rushdb.com',
  text = 'Start Building Free',
  ...props
}: Props & ComponentPropsWithoutRef<typeof Button>) => {
  return (
    <Button {...props} as={Link} href={href}>
      {children ?? text}
    </Button>
  )
}
