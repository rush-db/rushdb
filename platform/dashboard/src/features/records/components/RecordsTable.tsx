import type { Property, DBRecord, DBRecordInstance } from '@rushdb/javascript-sdk'
import type { ReactNode } from 'react'

import { useStore } from '@nanostores/react'
import { Database } from 'lucide-react'
import { forwardRef, memo, useEffect, useMemo, useRef, useState } from 'react'

import type { PaginatorProps } from '~/elements/Paginator'
import type { THeadCellProps } from '~/elements/Table'

import { Checkbox } from '~/elements/Checkbox'
import { Dialog, DialogTitle } from '~/elements/Dialog'
import { Label } from '~/elements/Label'
import { NothingFound } from '~/elements/NothingFound'
import { Paginator } from '~/elements/Paginator'
import { Skeleton } from '~/elements/Skeleton'
import { DataCell, HeadCell, TableRow } from '~/elements/Table'
import { getLabelColor } from '~/features/labels'
import { useProjectLabelsQuery } from '~/features/projects/hooks/useProjectQueries'
import { $recordsOrderBy, setRecordsSort } from '~/features/projects/stores/current-project'
import { $hiddenFields } from '~/features/projects/stores/hidden-fields'
import { $rowDensity } from '~/features/projects/stores/row-density'
import { collectPropertiesFromRecord } from '~/features/projects/utils'
import { PropertyName } from '~/features/properties/components/PropertyName'
import { PropertyValue } from '~/features/properties/components/PropertyValue'
import { handlePointerEnter, handlePointerLeave } from '~/features/properties/components/PropertyValueTooltip'
import { RecordsBatchActionsBar } from '~/features/records/components/RecordsBatchActionsBar'
import { cn, composeEventHandlers, range } from '~/lib/utils'

import { $selectedRecords, resetRecordsSelection, toggleRecordSelection } from '../stores/actionbar'
import {
  $selectedRelatedRecords,
  resetRelatedRecordsSelection,
  toggleRelatedRecordSelection
} from '~/features/records/stores/related-actionbar.ts'

type TableRecord = DBRecordInstance | DBRecord | Record<string, unknown>
type ShapedRow = Record<string, unknown>
type RecordsTableProps = TPolymorphicComponentProps<
  'div',
  {
    fields?: Property[]
    loading?: boolean
    onRecordClick?: (record: DBRecord) => void
    records?: TableRecord[]
    view?: 'related' | 'main'
  } & PaginatorProps
>

const INTERNAL_ID_FIELD: Property = {
  id: '__id',
  type: 'string',
  name: '__id'
}

const SCORE_FIELD: Property = {
  id: '__score',
  type: 'number',
  name: '__score'
}

function propertyKey({ name, type }: Pick<Property, 'name' | 'type'>) {
  return `${type}:${name}`
}

// Rendering a full page of rows (up to 1000) with many columns in one pass is a single
// long task that freezes the tab on switch/paginate. Instead, render an initial chunk
// that fills the viewport immediately, then append the rest in per-frame batches so the
// browser can paint and stay responsive between batches. Already-rendered rows are memoized
// (keyed by record id), so each batch only mounts the newly revealed rows.
const INITIAL_VISIBLE_ROWS = 40
const ROW_RENDER_BATCH = 60

function useProgressiveCount(total: number, resetKey: unknown) {
  const [count, setCount] = useState(() => Math.min(INITIAL_VISIBLE_ROWS, total))

  // Restart from the initial chunk whenever the underlying data changes (new page/query).
  useEffect(() => {
    setCount(Math.min(INITIAL_VISIBLE_ROWS, total))
  }, [resetKey, total])

  useEffect(() => {
    if (count >= total) return
    const raf = requestAnimationFrame(() => {
      setCount((current) => Math.min(current + ROW_RENDER_BATCH, total))
    })
    return () => cancelAnimationFrame(raf)
  }, [count, total])

  return count
}

function unwrapRow(record: TableRecord): ShapedRow {
  if (
    record &&
    typeof record === 'object' &&
    'data' in record &&
    record.data &&
    typeof record.data === 'object'
  ) {
    return record.data as ShapedRow
  }

  return record as ShapedRow
}

function isDbRecord(record: TableRecord) {
  const data = unwrapRow(record)
  return Boolean(data.__id && data.__proptypes)
}

function getRecordData(record: TableRecord): DBRecord {
  return unwrapRow(record) as DBRecord
}

function getRecordLabel(record: TableRecord) {
  const data = getRecordData(record)

  if (typeof data.__label === 'string') {
    return data.__label
  }

  if (
    record &&
    typeof record === 'object' &&
    'label' in record &&
    typeof (record as { label?: unknown }).label === 'string'
  ) {
    return (record as { label: string }).label
  }

  return 'RECORD'
}

function getRecordDate(record: TableRecord) {
  if (
    record &&
    typeof record === 'object' &&
    'date' in record &&
    (record as { date?: unknown }).date instanceof Date
  ) {
    return (record as { date: Date }).date
  }

  return new Date()
}

function isNestedValue(value: unknown) {
  return Boolean(value && typeof value === 'object')
}

function formatShapedValue(value: unknown) {
  if (value === null || typeof value === 'undefined') return '—'
  if (Array.isArray(value)) return `[${value.length}]`
  if (typeof value === 'object') return '{...}'
  return String(value)
}

function inferShapedColumns(rows: ShapedRow[]) {
  const columns: string[] = []

  for (const row of rows.slice(0, 50)) {
    for (const key of Object.keys(row)) {
      if (key === '__proptypes') continue
      if (!columns.includes(key)) {
        columns.push(key)
      }
      if (columns.length >= 32) {
        return columns
      }
    }
  }

  return columns
}

const ShapedRecordRow = memo(function ShapedRecordRow({
  columns,
  index,
  onClick,
  row,
  skip
}: {
  columns: string[]
  index: number
  onClick: (row: ShapedRow) => void
  row: ShapedRow
  skip: number
}) {
  return (
    <TableRow
      className={cn('group cursor-pointer transition-colors', {
        'bg-fill2': index % 2 === 0
      })}
      onClick={() => onClick(row)}
      style={{ height: 52.5 }}
      tabIndex={0}
    >
      <DataCell className="font-mono text-content2">{skip + index + 1}</DataCell>
      {columns.length ?
        columns.map((column) => {
          const value = row[column]
          return (
            <DataCell
              className={cn({
                'font-mono text-content2': isNestedValue(value),
                'text-content3': value === null || typeof value === 'undefined'
              })}
              key={`${index}-${column}`}
            >
              {formatShapedValue(value)}
            </DataCell>
          )
        })
      : <DataCell className="font-mono">{JSON.stringify(row)}</DataCell>}
    </TableRow>
  )
})

const ShapedRecordsTable = memo(function ShapedRecordsTable({
  className,
  limit,
  loading,
  onNext,
  onPrev,
  rows,
  skip,
  total,
  ...props
}: TPolymorphicComponentProps<
  'div',
  {
    loading?: boolean
    rows: ShapedRow[]
  } & PaginatorProps
>) {
  const tableRef = useRef<HTMLTableElement>(null)
  const scrollRef = useRef<HTMLDivElement>(null)
  const [rowDetails, setRowDetails] = useState<ShapedRow | undefined>()
  const columns = useMemo(() => inferShapedColumns(rows), [rows])
  const visibleRowCount = useProgressiveCount(rows.length, rows)
  const visibleRows = useMemo(() => rows.slice(0, visibleRowCount), [rows, visibleRowCount])

  const scrollToHead = () => {
    scrollRef.current?.scrollTo({ top: 0 })
  }

  if (!loading && rows.length === 0) {
    return <NothingFound />
  }

  return (
    <div className={cn('flex min-h-0 flex-1 flex-col overflow-hidden', className)}>
      <StickyHeaderWrapper ref={scrollRef} {...props}>
        <table className="w-full border-separate border-spacing-0" ref={tableRef}>
          <StickyTableHead>
            <tr className="group bg-fill">
              <HeadCell className="w-16">#</HeadCell>
              {columns.length ?
                columns.map((column) => (
                  <HeadCell key={column}>
                    <span className="truncate">{column}</span>
                  </HeadCell>
                ))
              : <HeadCell>Result</HeadCell>}
            </tr>
          </StickyTableHead>
          <tbody>
            {loading ?
              range(limit).map((index) => (
                <RecordRowSkeleton cols={Math.max(columns.length, 1)} key={index} />
              ))
            : visibleRows.map((row, index) => (
                <ShapedRecordRow
                  columns={columns}
                  index={index}
                  key={index}
                  onClick={setRowDetails}
                  row={row}
                  skip={skip}
                />
              ))
            }
          </tbody>
        </table>
      </StickyHeaderWrapper>

      {rows.length ?
        <Paginator
          className="shrink-0 border-t bg-fill"
          limit={limit}
          onNext={composeEventHandlers(scrollToHead, onNext)}
          onPrev={composeEventHandlers(scrollToHead, onPrev)}
          skip={skip}
          total={total}
        />
      : null}

      <Dialog
        className="sm:max-w-3xl"
        onOpenChange={(open) => {
          if (!open) setRowDetails(undefined)
        }}
        open={Boolean(rowDetails)}
      >
        <DialogTitle>Row JSON</DialogTitle>
        <pre className="max-h-[70vh] overflow-auto rounded-md border bg-fill2 p-4 font-mono text-xs leading-5 text-content">
          {JSON.stringify(rowDetails, null, 2)}
        </pre>
      </Dialog>
    </div>
  )
})

function FieldHeadCell({
  field,
  ...props
}: THeadCellProps & {
  field: Property
}) {
  const sort = useStore($recordsOrderBy)
  const sortField = typeof sort === 'string' ? '__id' : (sort ?? {})[field.name]
  const sortActive = typeof sort === 'string' ? field.name === '__id' : !!(sort ?? {})[field.name]
  const sortDirection = typeof sort === 'string' ? sort : (sort ?? {})[field.name]

  return (
    <HeadCell
      data-tour="records-table-overview"
      {...props}
      onClick={() => {
        setRecordsSort(field.name)
      }}
      sortActive={sortActive}
      sortDirection={sortDirection}
      sortField={sortField}
      sortable
    >
      <PropertyName
        name={field.name}
        sortActive={sortActive}
        sortDirection={sortDirection}
        sortable
        type={field.type}
      />
    </HeadCell>
  )
}

const RecordRowSkeleton = forwardRef<HTMLTableRowElement, { cols?: number }>(({ cols = 1 }, ref) => {
  return (
    <TableRow ref={ref}>
      <DataCell className="text-content-secondary">
        <Database size={16} />
      </DataCell>
      {[...Array(cols)].map((_, idx) => (
        <DataCell key={idx}>
          <Skeleton enabled>Loading...</Skeleton>
        </DataCell>
      ))}
    </TableRow>
  )
})
RecordRowSkeleton.displayName = 'RecordRowSkeleton'

const StickyHeaderWrapper = forwardRef<
  HTMLDivElement,
  {
    children: ReactNode
    className?: string
  } & Omit<TPolymorphicComponentProps<'div'>, 'children'>
>(({ children, className, ...props }, ref) => (
  <div {...props} className={cn('min-h-0 flex-1 overflow-auto', className)} ref={ref}>
    {children}
  </div>
))
StickyHeaderWrapper.displayName = 'StickyHeaderWrapper'

const StickyTableHead = ({ children }: { children: ReactNode }) => (
  <thead className="sticky top-0 z-10">{children}</thead>
)

const RecordRow = memo(function RecordRow({
  compact,
  fields,
  hiddenId,
  index,
  labelNames,
  loading,
  onRecordClick,
  record,
  selected,
  view
}: {
  compact: boolean
  fields: Property[]
  hiddenId: boolean
  index: number
  labelNames: string[]
  loading?: boolean
  onRecordClick?: (record: DBRecord) => void
  record: TableRecord
  selected: boolean
  view: 'related' | 'main'
}) {
  const recordData = getRecordData(record)
  const recordId = recordData.__id
  const recordLabel = getRecordLabel(record)
  const labelVariant = getLabelColor(recordLabel, labelNames.indexOf(recordLabel))
  const propertyMap = useMemo(() => {
    const next = new Map<string, ReturnType<typeof collectPropertiesFromRecord>[number]>()

    for (const property of collectPropertiesFromRecord(recordData)) {
      next.set(propertyKey(property), property)
    }

    return next
  }, [recordData])

  return (
    <TableRow
      className={cn('group cursor-pointer transition-colors', {
        'bg-fill2': index % 2 === 0
      })}
      onClick={onRecordClick ? () => onRecordClick(recordData) : undefined}
      style={{ height: compact ? 36 : 52.5 }}
      tabIndex={0}
    >
      <DataCell className={cn('text-content-secondary', compact && 'py-1')}>
        <Checkbox
          className={cn('hidden group-hover:block', {
            'block!': selected
          })}
          onCheckedChange={() =>
            view === 'main' ?
              toggleRecordSelection({
                recordId,
                selected
              })
            : toggleRelatedRecordSelection({
                recordId,
                selected
              })
          }
          checked={selected}
          onClick={(e) => e.stopPropagation()}
        />
        <Database
          className={cn('text-content2 group-hover:hidden', {
            'hidden!': selected
          })}
          size={16}
        />
      </DataCell>
      {!hiddenId && (
        <DataCell
          onPointerEnter={handlePointerEnter({
            property: {
              name: '__id',
              type: 'string',
              value: recordId,
              date: getRecordDate(record).toISOString()
            },
            showOperations: false
          })}
          className="py-1"
          onPointerLeave={handlePointerLeave}
        >
          <Skeleton enabled={loading}>
            <Label variant={labelVariant}>{recordLabel}</Label>

            {!compact && <PropertyValue className="text-content2" type={'string'} value={recordId} />}
          </Skeleton>
        </DataCell>
      )}

      {fields.map((field) => {
        const property =
          field.name === '__score' ?
            typeof recordData.__score === 'number' ?
              {
                name: '__score',
                type: 'number' as const,
                value: recordData.__score
              }
            : undefined
          : propertyMap.get(propertyKey(field))

        if (!property) {
          return (
            <DataCell
              className={cn('text-content3', compact && 'py-1')}
              key={`${recordId}-${field.id}-${field.name}`}
            >
              —
            </DataCell>
          )
        }

        return (
          <DataCell
            className={cn(compact && 'py-1')}
            key={`${recordId}-${property.name}`}
            onPointerEnter={handlePointerEnter({ property })}
            onPointerLeave={handlePointerLeave}
          >
            <Skeleton enabled={loading}>
              <PropertyValue type={property.type} value={property.value} />
            </Skeleton>
          </DataCell>
        )
      })}
    </TableRow>
  )
})

function RecordsTableComponent({
  className,
  records,
  loading,
  fields,
  total,
  limit,
  skip,
  onNext,
  onPrev,
  onRecordClick,
  view = 'main',
  ...props
}: RecordsTableProps) {
  const tableRef = useRef<HTMLTableElement>(null)
  const scrollRef = useRef<HTMLDivElement>(null)
  const hiddenFields = useStore($hiddenFields)
  const compact = useStore($rowDensity) === 'compact'
  const { data: labels } = useProjectLabelsQuery()
  const labelNames = useMemo(() => Object.keys(labels ?? {}), [labels])
  const hiddenId = hiddenFields.includes('__id')

  const scrollToHead = () => {
    scrollRef.current?.scrollTo({ top: 0 })
  }

  const selectedRecords = useStore($selectedRecords)
  const recordRows = useMemo(() => records?.filter(isDbRecord) ?? [], [records])
  const visibleRowCount = useProgressiveCount(recordRows.length, recordRows)
  const visibleRecordRows = useMemo(() => recordRows.slice(0, visibleRowCount), [recordRows, visibleRowCount])
  const hasOnlyDbRecords = useMemo(() => !records || records.every(isDbRecord), [records])
  const pageRecordIds = useMemo(() => recordRows.map((record) => getRecordData(record).__id), [recordRows])
  const hasScore = useMemo(
    () => recordRows.some((record) => typeof getRecordData(record).__score === 'number'),
    [recordRows]
  )
  const visibleFields = useMemo(
    () =>
      fields ?
        [...(hasScore && !fields.some((field) => field.name === '__score') ? [SCORE_FIELD] : []), ...fields]
      : fields,
    [fields, hasScore]
  )
  const selectedRecordsOnPage = pageRecordIds.filter((id) => selectedRecords.includes(id))
  const hasSelection = selectedRecordsOnPage.length > 0
  const allSelectedOnPage = pageRecordIds.length > 0 && selectedRecordsOnPage.length === pageRecordIds.length
  const mixedSelection = hasSelection && !allSelectedOnPage

  const selectedRelatedRecords = useStore($selectedRelatedRecords)
  const selectedRelatedRecordsOnPage = pageRecordIds.filter((id) => selectedRelatedRecords.includes(id))
  const hasRelatedSelection = selectedRelatedRecordsOnPage.length > 0
  const allRelatedSelectedOnPage =
    pageRecordIds.length > 0 && selectedRelatedRecordsOnPage.length === pageRecordIds.length
  const mixedRelatedSelection = hasRelatedSelection && !allRelatedSelectedOnPage

  const selectedRecordIds = view === 'main' ? selectedRecords : selectedRelatedRecords

  const checked = view === 'main' ? allSelectedOnPage : allRelatedSelectedOnPage
  const mixed = view === 'main' ? mixedSelection : mixedRelatedSelection

  const handleOnCheckedChange = () => {
    if (view === 'main') {
      if (allSelectedOnPage) {
        resetRecordsSelection()
      } else {
        $selectedRecords.set([...pageRecordIds])
      }
    }

    if (view === 'related') {
      if (allRelatedSelectedOnPage) {
        resetRelatedRecordsSelection()
      } else {
        $selectedRelatedRecords.set([...pageRecordIds])
      }
    }
  }

  if (!loading && Array.isArray(records) && records.length === 0) {
    return <NothingFound />
  }

  if (!loading && records && !hasOnlyDbRecords) {
    return (
      <ShapedRecordsTable
        className={className}
        limit={limit}
        loading={loading}
        onNext={onNext}
        onPrev={onPrev}
        rows={records.map(unwrapRow)}
        skip={skip}
        total={total}
        {...props}
      />
    )
  }

  return (
    <div className={cn('flex min-h-0 flex-1 flex-col overflow-hidden', className)}>
      <StickyHeaderWrapper ref={scrollRef} {...props}>
        <table className="w-full border-separate border-spacing-0" ref={tableRef}>
          <StickyTableHead>
            <tr className="group bg-fill">
              <HeadCell style={{ width: 44 }}>
                <Checkbox
                  className={cn('hidden group-hover:block', {
                    'block!': checked
                  })}
                  onCheckedChange={handleOnCheckedChange}
                  checked={checked}
                  mixed={mixed}
                />
              </HeadCell>
              {!hiddenId && <FieldHeadCell field={INTERNAL_ID_FIELD} key={INTERNAL_ID_FIELD.id} />}

              {visibleFields ?
                visibleFields?.map((field) => <FieldHeadCell field={field} key={field.id} />)
              : range(1).map((key) => (
                  <HeadCell key={key}>
                    <Skeleton enabled>
                      <PropertyName name={'Loading'} type="string" />
                    </Skeleton>
                  </HeadCell>
                ))
              }
            </tr>
          </StickyTableHead>
          <tbody>
            {(!records || !visibleFields) &&
              range(records?.length ?? limit).map((index) => <RecordRowSkeleton key={index} />)}

            {visibleFields &&
              visibleRecordRows &&
              visibleRecordRows?.map((record, index) => {
                if (!record) {
                  return null
                }

                const recordId = getRecordData(record).__id

                return (
                  <RecordRow
                    compact={compact}
                    fields={visibleFields}
                    hiddenId={hiddenId}
                    index={index}
                    key={recordId}
                    labelNames={labelNames}
                    loading={loading}
                    onRecordClick={onRecordClick}
                    record={record}
                    selected={selectedRecordIds.includes(recordId)}
                    view={view}
                  />
                )
              })}
          </tbody>
        </table>
      </StickyHeaderWrapper>

      <RecordsBatchActionsBar view={view} />

      {records?.length ?
        <Paginator
          className="shrink-0 border-t bg-fill"
          limit={limit}
          onNext={composeEventHandlers(scrollToHead, onNext)}
          onPrev={composeEventHandlers(scrollToHead, onPrev)}
          skip={skip}
          total={total}
        />
      : null}
    </div>
  )
}

export const RecordsTable = memo(RecordsTableComponent)
