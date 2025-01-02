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
      className={cn('hover:bg-secondary flex justify-between gap-5 p-2.5 sm:gap-10')}
      onPointerEnter={handlePointerEnter({ property, showOperations: false })}
      onPointerLeave={handlePointerLeave}
    >
      <PropertyName name={property.name} type={property.type} />
      <PropertyValue className="truncate text-end" type={property.type} value={property.value} />
    </li>
  )
}

function DataListItemSkeleton() {
  return (
    <li className="flex justify-between gap-5 py-2.5 first:pt-0 last:pb-0 sm:gap-10">
      <Skeleton enabled>
        <PropertyName name={'Loading...'} type="string" />
      </Skeleton>
      <Skeleton enabled>Loading...</Skeleton>
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
