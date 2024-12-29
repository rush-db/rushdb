import type { ReactNode } from 'react'

import { X } from 'lucide-react'
import { useState } from 'react'

import { Button } from './Button'
import { IconButton } from './IconButton'

export function GlobalNotification({
  action,
  children,
  description,
  title
}: {
  action?: ReactNode
  children?: ReactNode
  description?: ReactNode
  title?: ReactNode
}) {
  const [open, setOpen] = useState(true)

  if (!open) {
    return null
  }

  return (
    <div className="relative flex min-h-[36px] w-full flex-col items-center justify-start bg-warning px-5 py-2 text-center text-xs text-warning-contrast sm:flex-row sm:gap-3">
      {title ? <h3 className="font-semibold">{title}</h3> : null}
      {description ? <p className="opacity-90">{description}</p> : null}
      {children}
      {action && (
        <div className="end-12 mt-2 sm:absolute sm:mt-0">{action}</div>
      )}
      <IconButton
        aria-label="Close notification"
        className="absolute end-2 top-0"
        onClick={() => setOpen(false)}
        size="small"
        variant="ghost"
      >
        <X />
      </IconButton>
    </div>
  )
}
