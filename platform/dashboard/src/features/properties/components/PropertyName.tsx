import type { Property, PropertyType } from '@rushdb/javascript-sdk'

import { KeyRound } from 'lucide-react'

import type { SortIconProps, SortingProps } from '~/elements/Table'

import { Skeleton } from '~/elements/Skeleton'
import { SortIcon } from '~/elements/Table'
import { cn } from '~/lib/utils'

import { PropertyIconSquare, PropertyTypeIcon } from './PropertyTypeIcon'

export function PropertyName({
  className,
  type,
  name,
  iconSize = 16,
  sortable,
  sortActive,
  sortDirection,
  ...props
}: TPolymorphicComponentProps<
  'div',
  {
    iconSize?: number
    name: Property['name']
    type: PropertyType | undefined
  } & SortingProps &
    SortIconProps
>) {
  // __id is the record's primary key, not a user-defined string — show a key icon.
  const isPrimaryKey = name === '__id'

  return (
    <div
      className={cn(
        'text-content-secondary inline-flex shrink-0 items-center gap-1.5 leading-[0]',
        className
      )}
      {...props}
      title={isPrimaryKey ? 'Record ID (primary key)' : `Property "${name}": ${type}`}
    >
      <Skeleton enabled={!type}>
        {isPrimaryKey ?
          <PropertyIconSquare className="bg-secondary text-content2" size={iconSize}>
            <KeyRound size={Math.round(iconSize * 0.7)} />
          </PropertyIconSquare>
        : <PropertyTypeIcon size={iconSize} type={type ?? 'string'} />}
      </Skeleton>
      {name}
      {sortable && <SortIcon sortActive={sortActive} sortDirection={sortDirection} />}
    </div>
  )
}
