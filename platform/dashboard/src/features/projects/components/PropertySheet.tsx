import { useStore } from '@nanostores/react'
import { X } from 'lucide-react'

import { IconButton } from '~/elements/IconButton'
import { PageHeader } from '~/elements/PageHeader'
import { Close, Sheet } from '~/elements/Sheet'
import { DialogTitle } from '~/elements/Dialog'
import { Badge } from '~/elements/Badge'
import { Tab, Tabs, TabsContent, TabsList } from '~/elements/Tabs'

import { $sheetProperty } from '../stores/id'

function SectionTitle({ children }: { children: React.ReactNode }) {
  return <h4 className="text-content-secondary text-xs font-bold uppercase tracking-wide">{children}</h4>
}

const tabs = ['data', 'records'] as const
type PropertySheetTab = (typeof tabs)[number]

export function PropertySheet() {
  const property = useStore($sheetProperty)

  return (
    <Sheet
      open={property !== undefined}
      onOpenChange={(open) => {
        if (!open) {
          $sheetProperty.set(undefined)
        }
      }}
    >
      <PageHeader className="bg-fill2 sticky top-0 z-40 justify-start gap-3 px-5 py-3">
        <DialogTitle className="flex-1 truncate text-xl">{property?.name ?? 'Property'}</DialogTitle>
        <Close asChild>
          <IconButton aria-label="close" variant="ghost" size="small">
            <X />
          </IconButton>
        </Close>
      </PageHeader>

      <Tabs defaultValue={'data' satisfies PropertySheetTab}>
        <TabsList className="bg-fill2 w-full border-b px-5">
          {tabs.map((tab) => (
            <Tab key={tab} value={tab}>
              {tab}
            </Tab>
          ))}
        </TabsList>

        <TabsContent className="m-5" value="data">
          {property && (
            <div className="space-y-5">
              <div className="space-y-2">
                <SectionTitle>Name</SectionTitle>
                <div className="text-lg font-semibold">{property.name}</div>
              </div>

              <div className="space-y-2">
                <SectionTitle>Type</SectionTitle>
                <Badge>{property.type}</Badge>
              </div>

              <div className="space-y-2">
                <SectionTitle>Labels</SectionTitle>
                <div className="flex flex-wrap gap-2">
                  {property.labels.length ?
                    property.labels.map((label) => <Badge key={label}>{label}</Badge>)
                  : <span className="text-content-secondary text-sm">No labels in current view</span>}
                </div>
              </div>

              <div className="space-y-2">
                <SectionTitle>Embedding Index</SectionTitle>
                <span className="text-sm">
                  {property.vectorIndexed ? 'Enabled for at least one label' : 'Not indexed'}
                </span>
              </div>

              <div className="space-y-2">
                <SectionTitle>Connected Records (Visible)</SectionTitle>
                <span className="text-sm">{property.connectedRecordIds.length}</span>
              </div>

              <div className="space-y-2">
                <SectionTitle>Property Key</SectionTitle>
                <code className="bg-fill2 block break-all rounded-md p-2 text-xs">{property.key}</code>
              </div>
            </div>
          )}
        </TabsContent>

        <TabsContent className="m-5" value="records">
          {property && (
            <div className="space-y-4">
              <div className="text-content-secondary text-sm">
                {property.connectedRecordIds.length} connected records in the current graph view.
              </div>

              <div className="max-h-[420px] space-y-2 overflow-auto pr-1">
                {property.connectedRecordIds.length ?
                  property.connectedRecordIds.map((id) => (
                    <code key={id} className="bg-fill2 block break-all rounded-md p-2 text-xs">
                      {id}
                    </code>
                  ))
                : <span className="text-content-secondary text-sm">No connected records in current view</span>
                }
              </div>
            </div>
          )}
        </TabsContent>
      </Tabs>
    </Sheet>
  )
}
