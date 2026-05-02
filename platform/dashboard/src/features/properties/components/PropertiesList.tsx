import type { PropertyWithValue } from '@rushdb/javascript-sdk'

import { NothingFound } from '~/elements/NothingFound'
import { Skeleton } from '~/elements/Skeleton'
import { cn, range } from '~/lib/utils'

import { PropertyName } from './PropertyName'
import { PropertyValue } from './PropertyValue'
import { handlePointerEnter, handlePointerLeave } from './PropertyValueTooltip'

function DataListItem({ property }: { property: Pick<PropertyWithValue, 'name' | 'type' | 'value'> }) {
  return (
    <li
      className={cn('hover:bg-secondary flex items-center justify-between gap-5 px-5 py-3 sm:gap-10')}
      onPointerEnter={handlePointerEnter({ property, showOperations: false })}
      onPointerLeave={handlePointerLeave}
    >
      <PropertyName className="text-sm" name={property.name} type={property.type} />
      <PropertyValue className="text-sm font-medium truncate text-end" type={property.type} value={property.value} />
    </li>
  )
}

function DataListItemSkeleton() {
  return (
    <li className="flex items-center justify-between gap-5 px-5 py-3 sm:gap-10">
      <Skeleton enabled>
        <PropertyName className="text-sm" name={'Loading...'} type="string" />
      </Skeleton>
      <Skeleton enabled className="text-sm">Loading...</Skeleton>
    </li>
  )
}

export function PropertiesList({
  loading,
  properties
}: {
  loading?: boolean
  properties?: Array<Pick<PropertyWithValue, 'name' | 'type' | 'value'>>
}) {
  return (
    <ul
      className={
        'divide-stroke-tertiary flex w-full flex-col divide-y overflow-auto first:rounded-t last:rounded-b'
      }
    >
      {loading && range(5).map((num) => <DataListItemSkeleton key={num} />)}

      {!loading &&
        properties?.map((property, index) => <DataListItem key={`${property.name}`} property={property} />)}

      {!loading && !properties?.length && <NothingFound title="This findById has no data" />}
    </ul>
  )
}
