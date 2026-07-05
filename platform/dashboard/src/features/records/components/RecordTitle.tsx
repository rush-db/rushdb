import { cn } from '~/lib/utils'

import { Label } from '~/elements/Label.tsx'
import { getLabelColor } from '~/features/labels'
import { useProjectLabelsQuery } from '~/features/projects/hooks/useProjectQueries'

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
  const { data: labels } = useProjectLabelsQuery()
  const labelNames = Object.keys(labels ?? {})
  const labelVariant = getLabelColor(props.label, labelNames.indexOf(props.label))

  return (
    <div className={cn('flex flex-col gap-1', className)}>
      <Label variant={labelVariant}>{props.label}</Label>
      <div className="text-sm font-medium text-content2">{props.id}</div>
      {createdAt && (
        <div className="text-xs font-normal text-content2">Created at: {createdAt.toLocaleString()}</div>
      )}
    </div>
  )
}
