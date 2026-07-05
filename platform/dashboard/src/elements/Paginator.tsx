import { Check, ChevronLeft, ChevronRight } from 'lucide-react'

import { clamp, cn } from '~/lib/utils'

import { IconButton } from './IconButton'
import { inputWrapper } from './Input'
import { Menu, MenuIcon, MenuItem } from './Menu'
import { $currentProjectRecordsLimit } from '~/features/projects/stores/current-project.ts'

const LIMIT_OPTIONS = [50, 100, 250, 500, 1000]

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
    <div className={cn('flex items-center gap-4 py-2 pr-5 pl-1.5', className)} {...props}>
      <div className="flex gap-x-4">
        <Menu
          align="start"
          className="min-w-24"
          trigger={
            <button
              aria-label="Records per page"
              title="Records per page"
              className={cn(inputWrapper({ size: 'medium' }), 'w-20 cursor-pointer justify-between')}
            >
              {limit}
              <MenuIcon size={16} />
            </button>
          }
        >
          {LIMIT_OPTIONS.map((option) => (
            <MenuItem
              aria-selected={option === limit}
              className="h-9 flex-row px-3"
              key={option}
              onSelect={() => $currentProjectRecordsLimit.set(option)}
            >
              {option}
              <Check className={cn(option !== limit && 'invisible')} />
            </MenuItem>
          ))}
        </Menu>
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

      <div className="text-end text-sm text-content2">
        {total > 1 ? 'Records' : 'Record'} <strong>{clamp(1, total, skip)}</strong> -{' '}
        <strong>{clamp(1, total, skip + limit)}</strong> of <strong>{total}</strong>
      </div>
    </div>
  )
}
