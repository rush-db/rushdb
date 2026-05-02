import { cn } from '~/lib/utils'

import { Label } from '~/elements/Label.tsx'

export function RecordTitle({
  className,
  createdAt,
  ...props
}: {
  className?: string
  id: string
  label: string
  createdAt?: Date
}) {
  return (
    <div className={cn('flex flex-col gap-1', className)}>
      <Label>{props.label}</Label>
      <div className="text-content-secondary font-mono text-sm">{props.id}</div>
      {createdAt && (
        <div className="text-content-secondary text-xs">
          Created at: {createdAt.toLocaleString()}
        </div>
      )}
    </div>
  )
}
