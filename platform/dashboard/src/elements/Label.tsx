import type { PropsWithChildren } from 'react'
import type { VariantProps } from 'class-variance-authority'

import type { filterLabel } from '~/features/labels/components/FilterLabel'

import { variants } from '~/features/labels/components/FilterLabel'
import { cn } from '~/lib/utils'

type LabelVariant = keyof typeof variants

export const Label = ({
  children,
  className,
  variant,
  ...props
}: PropsWithChildren & TPolymorphicComponentProps<'div'> & VariantProps<typeof filterLabel>) => (
  <div
    className={cn(
      className,
      'bg-secondary text-content w-fit rounded-sm px-1 text-xs',
      variant && variants[variant as LabelVariant]
    )}
    {...props}
  >
    {children}
  </div>
)
