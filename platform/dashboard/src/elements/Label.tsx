import type { PropsWithChildren } from 'react'

import { cn } from '~/lib/utils'

export const Label = ({
  children,
  className,
  ...props
}: PropsWithChildren & TPolymorphicComponentProps<'div'>) => (
  <div
    className={cn(
      className,
      'text-content-secondary w-fit rounded-sm bg-accent/30 px-1 text-xs text-accent'
    )}
    {...props}
  >
    {children}
  </div>
)
