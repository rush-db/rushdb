import type { DBRecord } from '@rushdb/javascript-sdk'

import { useMemo } from 'react'

import { collectPropertiesFromRecord } from '~/features/projects/utils'
import { PropertiesList } from '~/features/properties/components/PropertiesList'

import { useCurrentRecordQuery } from '../hooks/useProjectQueries'

export function RecordDataTab() {
  const { data: record, isPending: loading } = useCurrentRecordQuery()
  const properties = useMemo(() => collectPropertiesFromRecord(record as unknown as DBRecord), [record])

  return <PropertiesList loading={loading} properties={properties} />
}
