import { cn } from '~/lib/utils'

import { Label } from '~/elements/Label.tsx'

export function RecordTitle({ className, ...props }: { className?: string; id: string; label: string }) {
  return (
    <div className={cn('flex flex-col gap-1', className)}>
      <Label>{props.label}</Label>
      <div className={cn('inline-flex items-center gap-1')}>{props.id}</div>
    </div>
  )
}
