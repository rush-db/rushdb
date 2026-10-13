import type { ReactNode } from 'react'

import { cn } from '~/lib/utils'

type Tone = 'neutral' | 'info' | 'active' | 'warning' | 'danger' | 'success'

const TONE_CLASSES: Record<Tone, string> = {
  neutral: 'bg-secondary text-content2',
  info: 'bg-badge-blue/15 text-badge-blue',
  active: 'bg-accent/15 text-accent',
  warning: 'bg-warning/15 text-warning',
  danger: 'bg-danger/15 text-danger',
  success: 'bg-success/15 text-success'
}

export function StatusBadge({
  tone = 'neutral',
  className,
  children
}: {
  tone?: Tone
  className?: string
  children: ReactNode
}) {
  return (
    <span
      className={cn(
        'inline-flex h-5 items-center gap-1 rounded-full px-2 text-2xs font-medium',
        TONE_CLASSES[tone],
        className
      )}
    >
      {children}
    </span>
  )
}

export function runStatusTone(status: string): Tone {
  switch (status) {
    case 'completed':
      return 'success'
    case 'completed_with_errors':
      return 'warning'
    case 'failed':
      return 'danger'
    case 'canceled':
      return 'neutral'
    case 'running':
    case 'finalizing':
      return 'active'
    case 'queued':
    case 'draft':
    case 'uploading':
      return 'info'
    case 'canceling':
      return 'warning'
    case 'blocked':
      return 'danger'
    default:
      return 'neutral'
  }
}

export function formatRunStatus(status: string): string {
  return status.replace(/_/g, ' ')
}
