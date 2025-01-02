// @ts-nocheck
import type { VariantProps } from 'class-variance-authority'

import { cva } from 'class-variance-authority'
import { forwardRef } from 'react'

import { cn } from '~/lib/utils'

export const variants = {
  blank: 'bg-badge-blue text-badge-blue-contrast',
  blue: 'bg-badge-blue text-badge-blue-contrast',
  green: 'bg-badge-green text-badge-green-contrast',
  orange: 'bg-badge-orange text-badge-orange-contrast',
  pink: 'bg-badge-pink text-badge-pink-contrast',
  red: 'bg-badge-red text-badge-red-contrast',
  yellow: 'bg-badge-yellow text-badge-yellow-contrast'
}

export const filterLabel = cva<{
  size: Record<string, string>
  variant: Record<string, string>
}>('inline-grid shrink-0 grid-flow-col place-items-center max-w-[180px] truncate', {
  variants: {
    variant: variants,
    size: {
      medium: 'h-9 rounded gap-2.5 px-4 text-xs font-bold',
      circle: 'h-1 w-1 rounded-full'
    }
  },
  defaultVariants: {
    size: 'medium',
    variant: 'blank'
  }
})

// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore
export const FilterLabel: TPolymorphicComponent<
  {
    active?: boolean
    quantity?: number | string
  } & VariantProps<typeof filterLabel>,
  'button'
> = forwardRef(({ active, children, className, quantity, size, type = 'button', variant, ...props }, ref) => {
  return (
    <button
      className={cn(filterLabel({ size, variant, className }), !active && 'opacity-30')}
      ref={ref}
      type={type}
      {...props}
    >
      <span>{children}</span>
      {quantity && <span className="text-content-secondary">{quantity}</span>}
    </button>
  )
})

FilterLabel.displayName = 'FilterLabel'
