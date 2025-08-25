import { useStore } from '@nanostores/react'
import { useEffect } from 'react'

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
import { Suspense, lazy } from 'react'
const GraphView = lazy(() => import('~/features/projects/components/GraphView.tsx'))
import { Spinner } from '~/elements/Spinner.tsx'
import { RawApiView } from '~/features/projects/components/RawApiView.tsx'
import { setTourStep } from '~/features/tour/stores/tour.ts'
import { $router } from '~/lib/router.ts'

function View() {
  const page = useStore($router)
  const { data: records, loading: loadingRecords, total = 0 } = useStore($filteredRecords)
  const { data: fields, loading: loadingFields } = useStore($currentProjectVisibleFields)

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
        <Suspense
          fallback={
            <div className="flex h-full w-full items-center justify-center py-10">
              <Spinner />
            </div>
          }
        >
          <GraphView />
        </Suspense>
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
