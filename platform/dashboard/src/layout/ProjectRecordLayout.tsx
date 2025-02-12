import { useStore } from '@nanostores/react'

import { NothingFound } from '~/elements/NothingFound'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Skeleton } from '~/elements/Skeleton'
import { RecordDataTab } from '~/features/projects/components/RecordDataTab'
import { RelatedRecordsTab } from '~/features/projects/components/RelatedRecordsTab.tsx'
import { $currentRecord } from '~/features/projects/stores/current-record'
import { PropertyValueTooltip } from '~/features/properties/components/PropertyValueTooltip'
import { RecordTitle } from '~/features/records/components/RecordTitle'
import { PageTab, PageTabs } from '~/layout/RootLayout/PageTabs'
import { $router, getRoutePath, isProjectRecordPage } from '~/lib/router'
import { ERecordSheetTabs } from '~/features/projects/types.ts'

function Routes() {
  const page = useStore($router)

  switch (page?.route) {
    case 'projectRecordRelations':
      return <RelatedRecordsTab />
    case 'projectRecord':
    default:
      return (
        <PageContent className="container flex flex-col justify-center pb-10">
          <RecordDataTab />
        </PageContent>
      )
  }
}

export function ProjectRecordLayout() {
  const page = useStore($router)
  const { data: record, loading } = useStore($currentRecord)
  // const { data: parents, loading: loadingParents } = useStore(
  //   $currentRecordParents
  // )

  if (!isProjectRecordPage(page)) {
    // redirectRoute('home')
    return null
  }

  if (!record && !loading) {
    return <NothingFound title="Record not found" />
  }

  const { id, recordId } = page.params

  return (
    <>
      <PageTabs className="w-full">
        <PageTab href={getRoutePath('projectRecord', { id, recordId })} label={ERecordSheetTabs.data} />
        <PageTab
          href={getRoutePath('projectRecordRelations', { id, recordId })}
          label={ERecordSheetTabs.relations}
        />
      </PageTabs>

      <PageHeader contained>
        <div className="flex flex-col">
          <PageTitle>
            <Skeleton enabled={loading}>
              <RecordTitle id={record!.__id as string} label={record!.__label} />
            </Skeleton>
          </PageTitle>
        </div>
      </PageHeader>

      <Routes />

      <PropertyValueTooltip />
    </>
  )
}
