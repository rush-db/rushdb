import type { DBRecord } from '@rushdb/javascript-sdk'

import { useStore } from '@nanostores/react'
import { useMemo } from 'react'

import { collectPropertiesFromRecord } from '~/features/projects/utils'
import { PropertiesList } from '~/features/properties/components/PropertiesList'

import { $currentRecord } from '../stores/current-record'

export function RecordDataTab() {
  const { data: record, loading } = useStore($currentRecord)
  const properties = useMemo(() => collectPropertiesFromRecord(record as unknown as DBRecord), [record])

  return <PropertiesList loading={loading} properties={properties} />
}
