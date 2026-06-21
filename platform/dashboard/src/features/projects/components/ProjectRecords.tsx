import type { DBRecord } from '@rushdb/javascript-sdk'

import { useStore } from '@nanostores/react'
import { useCallback, useEffect, useMemo } from 'react'

import { RecordSheet } from '~/features/projects/components/RecordSheet'
import { RecordsHeader } from '~/features/projects/components/RecordsHeader'
import { RecordsTable } from '~/features/records/components/RecordsTable'

import {
  $currentProjectRecordsLimit,
  $currentProjectRecordsSkip,
  $filteredRecordsTotal,
  $recordView,
  decrementRecordsPage,
  incrementRecordsPage
} from '../stores/current-project'
import { $hiddenFields, isFieldHidden } from '../stores/hidden-fields'
import { $sheetRecordId } from '../stores/id'
import { GraphView } from '~/features/projects/components/GraphView.tsx'
import { PropertySheet } from '~/features/projects/components/PropertySheet.tsx'
import { Paginator } from '~/elements/Paginator.tsx'
import { RawApiView } from '~/features/projects/components/RawApiView.tsx'
import { useFilteredRecordsQuery, useProjectFieldsQuery } from '~/features/projects/hooks/useProjectQueries'
import { $recordsSearchMode } from '~/features/projects/stores/records-search'
import { NothingFound } from '~/elements/NothingFound'

const isRecordResult = (record: unknown) => {
  const data =
    record && typeof record === 'object' && 'data' in record && typeof (record as any).data === 'object' ?
      (record as any).data
    : record

  return Boolean(data && typeof data === 'object' && (data as any).__id && (data as any).__proptypes)
}

function View() {
  const {
    data: recordsResult,
    error: recordsError,
    isError: recordsFailed,
    isPending: loadingRecords
  } = useFilteredRecordsQuery()
  const records = recordsResult?.data
  const searchMode = useStore($recordsSearchMode)
  const total =
    searchMode === 'semantic' && records?.length && !recordsResult?.total ?
      records.length
    : (recordsResult?.total ?? 0)

  useEffect(() => {
    $filteredRecordsTotal.set(total)
  }, [total])

  const { data: allFields, isPending: loadingFields } = useProjectFieldsQuery()
  const hiddenFields = useStore($hiddenFields)
  const fields = useMemo(
    () => allFields?.filter((field) => !isFieldHidden(hiddenFields, field.id)),
    [allFields, hiddenFields]
  )

  const skip = useStore($currentProjectRecordsSkip)

  const limit = useStore($currentProjectRecordsLimit)
  const openRecordSheet = useCallback((record: DBRecord) => {
    $sheetRecordId.set(record.__id)
  }, [])

  const shapedResults = Boolean(records?.length && !records.every(isRecordResult))
  const loading = !recordsFailed && (loadingRecords || (!shapedResults && loadingFields))

  const view = useStore($recordView)

  if (recordsFailed) {
    return (
      <NothingFound
        className="border-b"
        title={
          recordsError instanceof Error && recordsError.message ?
            `Search failed: ${recordsError.message}`
          : 'Search failed'
        }
      />
    )
  }

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
          onRecordClick={openRecordSheet}
          records={records}
          skip={skip}
          total={total}
        />
      )

    case 'graph':
      if (shapedResults) {
        return (
          <div className="grid min-h-0 flex-1 place-items-center border-b">
            <p className="text-content2 text-sm">
              Graph view is available for record results. This query returned shaped rows.
            </p>
          </div>
        )
      }

      return (
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <div className="min-h-0 flex-1 overflow-hidden">
            <GraphView />
          </div>
          {records?.length ?
            <Paginator
              className="bg-fill shrink-0 border-t"
              limit={limit}
              onNext={incrementRecordsPage}
              onPrev={decrementRecordsPage}
              skip={skip}
              total={total}
            />
          : null}
        </div>
      )

    default:
      return null
  }
}

export function ProjectRecords() {
  const view = useStore($recordView)

  if (view === 'raw-api') {
    return (
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <RecordsHeader />
        <RawApiView />
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <RecordsHeader />
      <View />
      <RecordSheet />
      <PropertySheet />
    </div>
  )
}
