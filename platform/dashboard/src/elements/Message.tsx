// @ts-nocheck
import type { VariantProps } from 'class-variance-authority'
import type { LucideIcon } from 'lucide-react'

import { cva } from 'class-variance-authority'
import { BadgeAlert } from 'lucide-react'
import { forwardRef } from 'react'

const message = cva('grid grid-flow-col', {
  variants: {
    variant: {
      danger: 'bg-danger/20 text-danger'
    },
    size: {
      medium: 'rounded px-2 py-1',
      small: 'rounded gap-3 px-3 py-1 min-h-[36px] text-sm gap-1 [&>svg]:mt-1.5 [&>svg]:w-4 [&>svg]:h-4'
    }
  },
  defaultVariants: {
    variant: 'danger',
    size: 'medium'
  }
})

const icons: Record<NonNullable<VariantProps<typeof message>['variant']>, LucideIcon> = {
  danger: BadgeAlert
}

type MessageProps = VariantProps<typeof message>

export const Message: TPolymorphicComponent<MessageProps, 'button'> = forwardRef(
  ({ className, as = 'div', children, variant, size, ...props }, ref) => {
    const Element = as
    // @ts-ignore
    const Icon: LucideIcon = icons[variant ?? 'danger']

    return (
      <Element className={message({ variant, size, className })} ref={ref} {...props}>
        <Icon />
        <div className="flex flex-wrap items-center gap-1">{children}</div>
      </Element>
    )
  }
)

Message.displayName = 'Message'
