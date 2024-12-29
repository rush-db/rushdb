import { useStore } from '@nanostores/react'

import { Banner } from '~/elements/Banner'
import { NothingFound } from '~/elements/NothingFound'
import { Spinner } from '~/elements/Spinner'
import { $currentProjectId } from '~/features/projects/stores/id'
import { openRoute } from '~/lib/router'

import { RecordsTable } from '../../records/components/RecordsTable'
import {
  $currentRecordChildrenLimit,
  $currentRecordChildrenSkip,
  $currentRecordFields,
  $currentRelatedRecords
} from '../stores/current-record'

export function RelatedRecordsTab() {
  const {
    data: records,
    loading: recordsLoading,
    total
  } = useStore($currentRelatedRecords)
  const { data: fields, loading: loadingFields } =
    useStore($currentRecordFields)
  // const skip = useStore($currentRecordChildrenSkip)
  // const limit = useStore($currentRecordChildrenLimit)
  // const projectId = useStore($currentProjectId)

  if (recordsLoading || loadingFields) {
    return <Banner image={<Spinner />} />
  }

  if (!records || !fields || !total) {
    return <NothingFound title="This Record doesn't have any related Records" />
  }

  return (
    <>
      {/*<RecordsTable*/}
      {/*  view="related"*/}
      {/*  onRecordClick={(record) =>*/}
      {/*    openRoute('projectRecord', {*/}
      {/*      id: projectId as string,*/}
      {/*      recordId: record.__id*/}
      {/*    })*/}
      {/*  }*/}
      {/*  className="mt-5"*/}
      {/*  fields={fields}*/}
      {/*  // TODO*/}
      {/*  limit={limit}*/}
      {/*  onNext={() => {}}*/}
      {/*  onPrev={() => {}}*/}
      {/*  records={records}*/}
      {/*  skip={skip}*/}
      {/*  total={total}*/}
      {/*/>*/}
      {JSON.stringify(records)}
    </>
  )
}
