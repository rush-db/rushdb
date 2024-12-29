import { SortAsc, SortDesc } from 'lucide-react'
import { forwardRef } from 'react'

import type { SortDirection } from '~/types'

import { cn } from '~/lib/utils'

export type SortIconProps = {
  sortActive?: boolean
  sortDirection?: SortDirection
}

export type SortingProps = { sortField?: number | string; sortable?: boolean }

export function SortIcon({ sortActive, sortDirection }: SortIconProps) {
  const icon =
    sortDirection === 'asc' ? <SortAsc size={16} /> : <SortDesc size={16} />

  return (
    <span
      className={cn(
        'text-content-tertiary inline-flex',
        !sortActive && 'opacity-0'
      )}
    >
      {icon}
    </span>
  )
}

export type THeadCellProps = TPolymorphicComponentProps<
  'th',
  SortingProps & SortIconProps
>

export function HeadCell({
  children,
  className,
  sortActive,
  sortDirection,
  sortField,
  sortable,
  ...props
}: THeadCellProps) {
  return (
    <th
      className={cn(
        'truncate bg-fill3 px-2 py-5 text-start align-middle font-mono text-xs font-medium first:pl-5 last:pr-5',
        {
          'cursor-pointer select-none hover:text-content': sortable,
          'text-content2 ': !sortActive,
          'font-semibold text-content': sortActive
        },
        className
      )}
      {...props}
    >
      <div className={cn('flex')}>{children}</div>
    </th>
  )
}

export function DataCell({
  className,
  ...props
}: TPolymorphicComponentProps<'td'>) {
  return (
    <td
      className={cn(
        'max-w-[150px] truncate px-2 py-4 text-start text-sm font-bold first:pl-5 last:pr-5',
        className
      )}
      {...props}
    />
  )
}

export const TableRow = forwardRef<
  HTMLTableRowElement,
  TPolymorphicComponentProps<'tr'>
>(({ className, ...props }, ref) => (
  <tr {...props} className={cn('hover:bg-secondary', className)} ref={ref} />
))
TableRow.displayName = 'TableRow'
