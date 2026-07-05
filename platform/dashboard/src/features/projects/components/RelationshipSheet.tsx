import type { PropertyType, PropertyWithValue } from '@rushdb/javascript-sdk'
import type { ReactNode } from 'react'

import { useStore } from '@nanostores/react'
import { ArrowRight, ArrowUpRight, GitFork, Info, List, X } from 'lucide-react'

import { Button } from '~/elements/Button'
import { DialogTitle } from '~/elements/Dialog'
import { IconButton } from '~/elements/IconButton'
import { Label } from '~/elements/Label'
import { PageHeader } from '~/elements/PageHeader'
import { Close, Sheet } from '~/elements/Sheet'
import { Tab, Tabs, TabsContent, TabsList } from '~/elements/Tabs'
import { getLabelColor } from '~/features/labels'
import { PropertiesList } from '~/features/properties/components/PropertiesList'

import { useProjectLabelsQuery } from '../hooks/useProjectQueries'
import { $sheetRelationship, openRecordSheet } from '../stores/id'

const tabs = [
  { value: 'overview', label: 'Overview', icon: <Info /> },
  { value: 'properties', label: 'Properties', icon: <List /> }
] as const
type RelationshipSheetTab = (typeof tabs)[number]['value']

// Relationship properties arrive untyped (Record<string, unknown>), so the
// display type can only be inferred from the runtime value.
function inferPropertyType(value: unknown): PropertyType {
  const sample = Array.isArray(value) ? value[0] : value
  if (typeof sample === 'number') return 'number'
  if (typeof sample === 'boolean') return 'boolean'
  return 'string'
}

function RecordLabelBadge({ label }: { label: string }) {
  const { data: labels } = useProjectLabelsQuery()
  const labelNames = Object.keys(labels ?? {})

  return <Label variant={getLabelColor(label, labelNames.indexOf(label))}>{label}</Label>
}

function SummaryRow({ children, title }: { children: ReactNode; title: string }) {
  return (
    <li className="flex items-center justify-between gap-5 px-4 py-3">
      <span className="shrink-0 font-mono text-xs font-medium text-content2">{title}</span>
      {children}
    </li>
  )
}

function EndpointCard({ id, label, title }: { id: string; label: string; title: string }) {
  return (
    <div className="flex min-w-0 flex-col gap-3 rounded-md border p-4">
      <div className="flex items-center justify-between gap-2">
        <span className="text-content-secondary text-xs font-semibold uppercase">{title}</span>
        <RecordLabelBadge label={label} />
      </div>
      <code className="min-w-0 font-mono text-xs break-all text-content2" title={id}>
        {id}
      </code>
      <Button
        className="mt-auto w-full items-center justify-center"
        variant="secondary"
        size="xsmall"
        onClick={() => openRecordSheet(id)}
      >
        <ArrowUpRight />
        Open record
      </Button>
    </div>
  )
}

export function RelationshipSheet() {
  const relation = useStore($sheetRelationship)

  const properties = Object.entries(relation?.properties ?? {}).map(([name, value]) => ({
    name,
    type: inferPropertyType(value),
    value: value as PropertyWithValue['value']
  }))

  return (
    <Sheet
      open={relation !== undefined}
      onOpenChange={(open) => {
        if (!open) $sheetRelationship.set(undefined)
      }}
    >
      <PageHeader className="sticky top-0 z-40 min-h-0 justify-start gap-3 bg-fill2 px-5 py-4">
        <DialogTitle className="flex-1 truncate text-xl">
          {relation && (
            <div className="flex min-w-0 flex-col gap-1">
              <div className="flex items-center gap-2">
                <GitFork className="size-4 shrink-0 text-content2" />
                <span className="truncate font-mono">{relation.type}</span>
              </div>
              <div className="text-xs font-normal text-content2">Relationship</div>
            </div>
          )}
        </DialogTitle>

        <Close asChild>
          <IconButton aria-label="close" variant="ghost" size="small">
            <X />
          </IconButton>
        </Close>
      </PageHeader>

      {relation && (
        <Tabs
          defaultValue={'overview' satisfies RelationshipSheetTab}
          key={`${relation.sourceId}:${relation.targetId}:${relation.type}`}
        >
          <TabsList className="w-full border-b bg-fill2 px-5">
            {tabs
              .filter(({ value }) => properties.length > 0 || value !== 'properties')
              .map(({ value, label, icon }) => (
                <Tab key={value} value={value}>
                  {icon}
                  {label}
                </Tab>
              ))}
          </TabsList>

          <TabsContent value={'overview' satisfies RelationshipSheetTab}>
            <div className="flex flex-col gap-4 p-5">
              <section className="rounded-md border">
                <h3 className="border-b px-4 py-3 text-sm font-semibold">Summary</h3>
                <ul className="divide-stroke-tertiary flex flex-col divide-y">
                  <SummaryRow title="type">
                    <span className="truncate font-mono text-sm font-medium">{relation.type}</span>
                  </SummaryRow>
                  <SummaryRow title="direction">
                    <span className="flex min-w-0 items-center gap-2">
                      <RecordLabelBadge label={relation.sourceLabel} />
                      <ArrowRight className="size-3.5 shrink-0 text-content3" />
                      <RecordLabelBadge label={relation.targetLabel} />
                    </span>
                  </SummaryRow>
                  {properties.length > 0 && (
                    <SummaryRow title="properties">
                      <span className="text-sm font-medium">{properties.length}</span>
                    </SummaryRow>
                  )}
                </ul>
              </section>

              <section
                aria-label="Relationship endpoints"
                className="grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-stretch gap-2"
              >
                <EndpointCard id={relation.sourceId} label={relation.sourceLabel} title="Source" />
                <ArrowRight className="size-4 shrink-0 self-center text-content3" />
                <EndpointCard id={relation.targetId} label={relation.targetLabel} title="Target" />
              </section>
            </div>
          </TabsContent>

          <TabsContent value={'properties' satisfies RelationshipSheetTab}>
            <PropertiesList properties={properties} />
          </TabsContent>
        </Tabs>
      )}
    </Sheet>
  )
}
