import type { ReactNode } from 'react'
import { useState, useEffect } from 'react'
import { useStore } from '@nanostores/react'
import { idToDate } from '@rushdb/javascript-sdk'
import { Copy, Database, GitFork, MoreVertical, Pencil, Trash, X } from 'lucide-react'

import { IconButton } from '~/elements/IconButton'
import { PageHeader } from '~/elements/PageHeader'
import { Close, Sheet } from '~/elements/Sheet'
import { Tab, Tabs, TabsContent, TabsList } from '~/elements/Tabs'
import { RecordTitle } from '~/features/records/components/RecordTitle'
import { useDeleteRecordMutation } from '~/features/records/hooks/useRecordMutations'

import { $sheetRecordId } from '../stores/id'
import { useCurrentRecordQuery, useCurrentRecordRelatedQuery } from '../hooks/useProjectQueries'
import { RecordDataTab } from './RecordDataTab'
import { RelatedRecordsTab } from './RelatedRecordsTab.tsx'
import { ERecordSheetTabs } from '~/features/projects/types.ts'
import { EditRecordDialog } from './EditRecordDialog'
import { MenuItem, Menu } from '~/elements/Menu.tsx'
import { copyToClipboard } from '~/lib/utils.ts'
import { Divider } from '~/elements/Divider.tsx'
import { DialogTitle } from '~/elements/Dialog.tsx'

const tabConfig: { value: ERecordSheetTabs; label: string; icon: ReactNode }[] = [
  { value: ERecordSheetTabs.data, label: 'Data', icon: <Database /> },
  { value: ERecordSheetTabs.relations, label: 'Relations', icon: <GitFork /> }
]

export function RecordSheet() {
  const id = useStore($sheetRecordId)
  const { data: record } = useCurrentRecordQuery()
  const { data: relationsResult } = useCurrentRecordRelatedQuery()
  const relations = relationsResult?.data
  const { mutate: deleteRecord } = useDeleteRecordMutation()

  const [activeTab, setActiveTab] = useState<ERecordSheetTabs>(ERecordSheetTabs.data)
  useEffect(() => {
    setActiveTab(ERecordSheetTabs.data)
  }, [id])

  return (
    <Sheet
      onOpenChange={(open) => {
        if (!open) {
          $sheetRecordId.set(undefined)
        }
      }}
      open={id !== undefined}
    >
      <PageHeader className="sticky top-0 z-40 min-h-0 justify-start gap-3 bg-fill2 px-5 py-4">
        <DialogTitle className="flex-1 truncate text-xl">
          {record && (
            <RecordTitle id={record.__id} label={record.__label} createdAt={idToDate(record.__id)} />
          )}
        </DialogTitle>

        {record && (
          <EditRecordDialog
            record={record}
            trigger={
              <IconButton aria-label="edit record" title="Edit record" variant="ghost" size="small">
                <Pencil />
              </IconButton>
            }
          />
        )}

        {record && (
          <Menu
            trigger={
              <IconButton aria-label="more" title="More" variant="ghost" size="small">
                <MoreVertical />
              </IconButton>
            }
            align="end"
          >
            <MenuItem
              icon={<Copy />}
              onClick={() => copyToClipboard(record.__id, { showSuccessToast: true })}
            >
              Copy Record ID
            </MenuItem>
            <Divider />
            <MenuItem icon={<Trash />} onClick={() => deleteRecord({ id: record.__id })} variant="danger">
              <Close>Delete Record</Close>
            </MenuItem>
          </Menu>
        )}
        <Close asChild>
          <IconButton aria-label="close" variant="ghost" size="small">
            <X />
          </IconButton>
        </Close>
      </PageHeader>

      <Tabs value={activeTab} onValueChange={(v) => setActiveTab(v as ERecordSheetTabs)}>
        <TabsList className="w-full border-b bg-fill2 px-5">
          {tabConfig
            .filter(({ value }) => relations || value !== ERecordSheetTabs.relations)
            .map(({ value, label, icon }) => (
              <Tab key={value} value={value}>
                {icon}
                {label}
              </Tab>
            ))}
        </TabsList>

        <TabsContent value={ERecordSheetTabs.data}>
          <RecordDataTab />
        </TabsContent>
        <TabsContent value={ERecordSheetTabs.relations}>
          <RelatedRecordsTab />
        </TabsContent>
      </Tabs>
    </Sheet>
  )
}
