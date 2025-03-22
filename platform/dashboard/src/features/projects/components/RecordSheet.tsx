import { useStore } from '@nanostores/react'
import { Copy, MoreVertical, Trash, X } from 'lucide-react'

import { IconButton } from '~/elements/IconButton'
import { PageHeader } from '~/elements/PageHeader'
import { Close, Sheet } from '~/elements/Sheet'
import { Tab, Tabs, TabsContent, TabsList } from '~/elements/Tabs'
import { RecordTitle } from '~/features/records/components/RecordTitle'
import { deleteRecordMutation } from '~/features/records/stores/mutations'

import { $currentRecord, $currentRelatedRecords } from '../stores/current-record'
import { $sheetRecordId } from '../stores/id'
import { RecordDataTab } from './RecordDataTab'
import { RelatedRecordsTab } from './RelatedRecordsTab.tsx'
import { ERecordSheetTabs } from '~/features/projects/types.ts'
import { MenuItem, Menu } from '~/elements/Menu.tsx'
import { copyToClipboard } from '~/lib/utils.ts'
import { Divider } from '~/elements/Divider.tsx'
import { DialogTitle } from '~/elements/Dialog.tsx'

const tabs: ERecordSheetTabs[] = [
  ERecordSheetTabs.data,
  // ERecordSheetTabs.api,
  ERecordSheetTabs.relations
]

export function RecordSheet() {
  const id = useStore($sheetRecordId)
  const { data: record } = useStore($currentRecord)
  const { data: relations } = useStore($currentRelatedRecords)
  const { mutate: deleteRecord } = useStore(deleteRecordMutation)

  return (
    <Sheet
      onOpenChange={(open) => {
        if (!open) {
          $sheetRecordId.set(undefined)
        }
      }}
      open={id !== undefined}
    >
      <PageHeader className="bg-fill2 sticky top-0 z-40 justify-start gap-5 px-5 py-3">
        <Close asChild>
          <IconButton aria-label="close" variant="ghost">
            <X />
          </IconButton>
        </Close>

        <DialogTitle className="flex-1 truncate text-xl">
          {record && <RecordTitle id={record.__id} label={record.__label} />}
        </DialogTitle>

        {record && (
          <Menu
            trigger={
              <IconButton aria-label="more" title="More" variant="ghost">
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
      </PageHeader>

      <Tabs defaultValue={ERecordSheetTabs.data}>
        <TabsList className="bg-fill2 w-full border-b px-5">
          {tabs
            .filter((t) => (!relations ? t !== ERecordSheetTabs.relations : t))
            .map((t) => (
              <Tab key={t} value={t}>
                {t}
              </Tab>
            ))}
        </TabsList>

        <TabsContent className="m-5" value={ERecordSheetTabs.data}>
          <RecordDataTab />
        </TabsContent>
        <TabsContent value={ERecordSheetTabs.relations}>
          <RelatedRecordsTab />
        </TabsContent>
      </Tabs>
    </Sheet>
  )
}
