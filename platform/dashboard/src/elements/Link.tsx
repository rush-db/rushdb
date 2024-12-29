import { forwardRef } from 'react'

import { cn } from '~/lib/utils'

const variants = {
  accent: 'text-accent hover:opacity-80 disabled:opacity-50',
  text: 'text-content hover:opacity-80 disabled:opacity-50'
}

const sizes = {
  medium: '[&>svg]:w-[24px] [&>svg]:h-[24px]',
  small: '[&>svg]:w-[16px] [&>svg]:h-[16px] text-sm',
  xsmall: '[&>svg]:w-[16px] [&>svg]:h-[16px] text-xs'
}

export const Link: TPolymorphicComponent<
  {
    size?: keyof typeof sizes
    variant?: keyof typeof variants
    disabled?: boolean
  },
  'a'
> = forwardRef(
  (
    {
      as = 'a',
      className,
      size = 'medium',
      variant = 'accent',
      disabled,
      ...props
    },
    ref
  ) => {
    const Element = as

    return (
      <Element
        className={cn(
          'inline-grid shrink-0 hover:underline',
          sizes[size],
          variants[variant],
          className
        )}
        disabled={disabled}
        ref={ref}
        {...props}
      />
    )
  }
)

Link.displayName = 'Link'
