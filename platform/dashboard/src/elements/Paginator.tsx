import { ChevronLeft, ChevronRight } from 'lucide-react'

import { clamp, cn } from '~/lib/utils'

import { IconButton } from './IconButton'
import { Select } from '~/elements/Select.tsx'
import { SelectItem } from '~/elements/SearchSelect.tsx'
import { useStore } from '@nanostores/react'
import { $currentProjectRecordsLimit } from '~/features/projects/stores/current-project.ts'

export type PaginatorProps = TInheritableElementProps<
  'div',
  {
    limit: number
    onNext: (event?: unknown) => void
    onPrev: (event?: unknown) => void
    skip: number
    total: number
  }
>

export function Paginator({ total, skip, limit, onNext, onPrev, className, ...props }: PaginatorProps) {
  return (
    <div className={cn('flex items-center gap-4 py-2 pl-1.5 pr-5', className)} {...props}>
      <div className="flex gap-x-4">
        <Select
          className="w-20"
          onChange={(value: { target: { value: any } }) => {
            $currentProjectRecordsLimit.set(Number(value.target.value))
          }}
          value={limit}
          options={[
            { value: 50, label: '50' },
            { value: 100, label: '100' },
            { value: 250, label: '250' },
            { value: 500, label: '500' },
            { value: 1000, label: '1000' }
          ]}
        />
        <IconButton aria-label="Previous page" disabled={skip <= 0} onClick={onPrev} title="Previous page">
          <ChevronLeft />
        </IconButton>
        <IconButton
          aria-label="Next page"
          disabled={skip >= total - limit}
          onClick={onNext}
          title="Next page"
        >
          <ChevronRight />
        </IconButton>
      </div>

      <div className="text-content2 text-end text-sm">
        {total > 1 ? 'Records' : 'Record'} <strong>{clamp(1, total, skip)}</strong> -{' '}
        <strong>{clamp(1, total, skip + limit)}</strong> of <strong>{total}</strong>
      </div>
    </div>
  )
}
