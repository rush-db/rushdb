import { useStore } from '@nanostores/react'
import { useEffect, useMemo } from 'react'

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
import { Paginator } from '~/elements/Paginator.tsx'
import { RawApiView } from '~/features/projects/components/RawApiView.tsx'
import { setTourStep } from '~/features/tour/stores/tour.ts'
import { $router } from '~/lib/router.ts'
import { useFilteredRecordsQuery, useProjectFieldsQuery } from '~/features/projects/hooks/useProjectQueries'

function View() {
  const page = useStore($router)
  const { data: recordsResult, isPending: loadingRecords } = useFilteredRecordsQuery()
  const records = recordsResult?.data
  const total = recordsResult?.total ?? 0

  useEffect(() => {
    $filteredRecordsTotal.set(recordsResult?.total)
  }, [recordsResult?.total])

  const { data: allFields, isPending: loadingFields } = useProjectFieldsQuery()
  const hiddenFields = useStore($hiddenFields)
  const fields = useMemo(
    () => allFields?.filter((field) => !isFieldHidden(hiddenFields, field.id)),
    [allFields, hiddenFields]
  )

  const skip = useStore($currentProjectRecordsSkip)

  const limit = useStore($currentProjectRecordsLimit)

  const loading = loadingRecords || loadingFields

  const view = useStore($recordView)

  useEffect(() => {
    if (page?.route === 'project' && !loading && records?.length) {
      setTourStep('recordTableOverview', false)
    }
  }, [page?.route, loading])

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
          {records?.length ?
            <Paginator
              className="bg-fill sticky bottom-0 border-t"
              limit={limit}
              onNext={incrementRecordsPage}
              onPrev={decrementRecordsPage}
              skip={skip}
              total={total}
            />
          : null}
        </>
      )

    default:
      return null
  }
}

export function ProjectRecords() {
  const view = useStore($recordView)

  if (view === 'raw-api') {
    return (
      <>
        <RecordsHeader />
        <RawApiView />
      </>
    )
  }

  return (
    <>
      <RecordsHeader />
      <View />
      <RecordSheet />
    </>
  )
}
