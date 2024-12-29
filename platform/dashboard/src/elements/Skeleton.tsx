import { forwardRef } from 'react'

import { cn } from '~/lib/utils'

export const Skeleton = forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement> & { enabled?: boolean }
>(({ className, enabled, ...props }, ref) => {
  return (
    <div
      className={cn(enabled ? 'skeleton inline-flex' : 'contents', className)}
      ref={ref}
      {...props}
    />
  )
})
Skeleton.displayName = 'Skeleton'
