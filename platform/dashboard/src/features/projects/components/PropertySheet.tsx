import { useStore } from '@nanostores/react'
import { ArrowUpRight, Database, List, X } from 'lucide-react'

import { IconButton } from '~/elements/IconButton'
import { PageHeader } from '~/elements/PageHeader'
import { Close, Sheet } from '~/elements/Sheet'
import { DialogTitle } from '~/elements/Dialog'
import { Tab, Tabs, TabsContent, TabsList } from '~/elements/Tabs'
import { Tooltip } from '~/elements/Tooltip.tsx'
import { PropertyTypeIcon } from '~/features/properties/components/PropertyTypeIcon'

import { $sheetProperty, $sheetRecordId } from '../stores/id'

const tabs = [
  { value: 'data', label: 'Data', icon: <Database /> },
  { value: 'records', label: 'Records', icon: <List /> }
] as const
type PropertySheetTab = (typeof tabs)[number]['value']

function openRecord(id: string) {
  $sheetProperty.set(undefined)
  $sheetRecordId.set(id)
}

export function PropertySheet() {
  const property = useStore($sheetProperty)

  return (
    <Sheet
      open={property !== undefined}
      onOpenChange={(open) => {
        if (!open) $sheetProperty.set(undefined)
      }}
    >
      <PageHeader className="bg-fill2 sticky top-0 z-40 min-h-0 justify-start gap-3 px-5 py-4">
        <DialogTitle className="flex-1 truncate text-xl">
          {property && (
            <div className="flex flex-col">
              <div className="flex flex-row items-center gap-1">
                <Tooltip trigger={<span className="inline-flex cursor-default"><PropertyTypeIcon size={18} type={property.type} /></span>}>
                  {property.type}
                </Tooltip>
                <span>{property.name}</span>
              </div>
              <div className="text-content-secondary font-mono text-sm">{property.id}</div>
            </div>
          )}
        </DialogTitle>

        <Close asChild>
          <IconButton aria-label="close" variant="ghost" size="small">
            <X />
          </IconButton>
        </Close>
      </PageHeader>

      <Tabs defaultValue={'data' satisfies PropertySheetTab}>
        <TabsList className="bg-fill2 w-full border-b px-5">
          {tabs.map(({ value, label, icon }) => (
            <Tab key={value} value={value}>
              {icon}
              {label}
            </Tab>
          ))}
        </TabsList>

        <TabsContent value="data">
          {property && (
            <ul className="divide-stroke-tertiary flex w-full flex-col divide-y">
              <li className="hover:bg-secondary flex items-center justify-between gap-5 px-5 py-3">
                <span className="text-content-secondary shrink-0 text-sm">Records</span>
                <span className="text-sm font-medium">
                  {property.recordsCount !== undefined ? property.recordsCount.toLocaleString() : '—'}
                </span>
              </li>

              <li className="hover:bg-secondary flex items-center justify-between gap-5 px-5 py-3">
                <span className="text-content-secondary shrink-0 text-sm">Embedding Index</span>
                <span className="text-end text-sm font-medium">
                  {property.vectorIndexed ? 'Enabled' : 'Not indexed'}
                </span>
              </li>
            </ul>
          )}
        </TabsContent>

        <TabsContent value="records">
          {property && (
            <div className="divide-stroke-tertiary flex flex-col divide-y">
              <div className="bg-fill2 text-content-secondary px-5 py-2 text-xs">
                {property.connectedRecordIds.length} of{' '}
                {property.recordsCount !== undefined ? property.recordsCount.toLocaleString() : '…'} total
              </div>

              {property.connectedRecordIds.length ?
                property.connectedRecordIds.map((id) => (
                  <div key={id} className="hover:bg-secondary flex items-center gap-3 px-5 py-3">
                    <code className="min-w-0 flex-1 break-all font-mono text-xs">{id}</code>
                    <IconButton
                      aria-label="open record"
                      title="Open record"
                      variant="ghost"
                      size="small"
                      onClick={() => openRecord(id)}
                    >
                      <ArrowUpRight />
                    </IconButton>
                  </div>
                ))
              : <div className="text-content-secondary px-5 py-3 text-sm">
                  No connected records in current view.
                </div>
              }
            </div>
          )}
        </TabsContent>
      </Tabs>
    </Sheet>
  )
}
