import type { PropsWithChildren } from 'react'

import { cn } from '~/lib/utils'

export const Label = ({
  children,
  className,
  ...props
}: PropsWithChildren & TPolymorphicComponentProps<'div'>) => (
  <div
    className={cn(className, 'text-content-secondary bg-accent/30 text-accent w-fit rounded-sm px-1 text-xs')}
    {...props}
  >
    {children}
  </div>
)
