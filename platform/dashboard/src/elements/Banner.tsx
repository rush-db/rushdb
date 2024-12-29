import type { ReactNode } from 'react'

import { cn } from '~/lib/utils'

export function Banner({
  className,
  title,
  action,
  image
}: TPolymorphicComponentProps<
  'div',
  { action?: ReactNode; image?: ReactNode; title?: string }
>) {
  return (
    <div
      className={cn(
        'flex min-h-[300px] flex-col items-center justify-center gap-3 py-5',
        className
      )}
    >
      {image && <div className="">{image}</div>}
      {title && <p className="font-bold">{title}</p>}
      {action && <div className="mt-2">{action}</div>}
    </div>
  )
}
