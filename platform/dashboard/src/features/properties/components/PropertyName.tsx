import type {
  CollectProperty,
  CollectPropertyType
} from '@collect.so/javascript-sdk'

import type { SortIconProps, SortingProps } from '~/elements/Table'

import { Skeleton } from '~/elements/Skeleton'
import { SortIcon } from '~/elements/Table'
import { cn } from '~/lib/utils'

import { PropertyTypeIcon } from './PropertyTypeIcon'

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
    name: CollectProperty['name']
    type: CollectPropertyType | undefined
  } & SortingProps &
    SortIconProps
>) {
  return (
    <div
      className={cn(
        'text-content-secondary inline-flex shrink-0 items-center gap-1.5 leading-[0]',
        className
      )}
      {...props}
      title={`Property "${name}": ${type}`}
    >
      <Skeleton enabled={!type}>
        <PropertyTypeIcon size={iconSize} type={type ?? 'string'} />
      </Skeleton>
      {name}
      {sortable && (
        <SortIcon sortActive={sortActive} sortDirection={sortDirection} />
      )}
    </div>
  )
}
