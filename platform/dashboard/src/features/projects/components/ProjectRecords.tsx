import { useStore } from '@nanostores/react'

import { RecordSheet } from '~/features/projects/components/RecordSheet'
import { RecordsHeader } from '~/features/projects/components/RecordsHeader'
import { RecordsTable } from '~/features/records/components/RecordsTable'

import {
  $currentProjectRecordsLimit,
  $currentProjectRecordsSkip,
  $filteredRecords,
  $recordView,
  decrementRecordsPage,
  incrementRecordsPage
} from '../stores/current-project'
import { $currentProjectVisibleFields } from '../stores/hidden-fields'
import { $sheetRecordId } from '../stores/id'
import { GraphView } from '~/features/projects/components/GraphView.tsx'
import { Paginator } from '~/elements/Paginator.tsx'
import { composeEventHandlers } from '~/lib/utils.ts'
import React from 'react'

function View() {
  const {
    data: records,
    loading: loadingRecords,
    total = 0
  } = useStore($filteredRecords)
  const { data: fields, loading: loadingFields } = useStore(
    $currentProjectVisibleFields
  )

  const skip = useStore($currentProjectRecordsSkip)

  const limit = useStore($currentProjectRecordsLimit)

  const loading = loadingRecords || loadingFields

  const view = useStore($recordView)

  switch (view) {
    case 'table':
      return (
        <RecordsTable
          className="w-full"
          fields={fields}
          limit={limit}
          loading={loading}
          onNext={incrementRecordsPage}
          onPrev={decrementRecordsPage}
          onRecordClick={(record) => $sheetRecordId.set(record.__id)}
          records={records}
          skip={skip}
          total={total}
        />
      )

    case 'graph':
      return (
        <>
          <GraphView />
          {records?.length ? (
            <Paginator
              className="sticky bottom-0 border-t bg-fill"
              limit={limit}
              onNext={incrementRecordsPage}
              onPrev={decrementRecordsPage}
              skip={skip}
              total={total}
            />
          ) : null}
        </>
      )

    default:
      return null
  }
}

export function ProjectRecords() {
  return (
    <>
      <RecordsHeader />
      <View />
      <RecordSheet />
    </>
  )
}