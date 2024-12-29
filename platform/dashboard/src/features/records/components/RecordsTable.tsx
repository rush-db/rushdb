import type { CollectProperty, CollectRecord } from '@collect.so/javascript-sdk'
import type { ReactElement, ReactNode, UIEventHandler } from 'react'

import { useStore } from '@nanostores/react'
import { Database } from 'lucide-react'
import { cloneElement, forwardRef, useEffect, useRef, useState } from 'react'

import type { PaginatorProps } from '~/elements/Paginator'
import type { THeadCellProps } from '~/elements/Table'

import { Checkbox } from '~/elements/Checkbox'
import { Label } from '~/elements/Label'
import { NothingFound } from '~/elements/NothingFound'
import { Paginator } from '~/elements/Paginator'
import { Skeleton } from '~/elements/Skeleton'
import { DataCell, HeadCell, TableRow } from '~/elements/Table'
import {
  $recordsOrderBy,
  setRecordsSort
} from '~/features/projects/stores/current-project'
import { $hiddenFields } from '~/features/projects/stores/hidden-fields'
import { collectPropertiesFromRecord } from '~/features/projects/utils'
import { PropertyName } from '~/features/properties/components/PropertyName'
import { PropertyValue } from '~/features/properties/components/PropertyValue'
import {
  handlePointerEnter,
  handlePointerLeave
} from '~/features/properties/components/PropertyValueTooltip'
import { RecordsBatchActionsBar } from '~/features/records/components/RecordsBatchActionsBar'
import { cn, composeEventHandlers, isInViewport, range } from '~/lib/utils'

import {
  $hasRecordsSelection,
  $mixedRecordsSelection,
  $selectedRecords,
  resetRecordsSelection,
  toggleRecordSelection
} from '../stores/actionbar'
import {
  $hasRelatedRecordsSelection,
  $mixedRelatedRecordsSelection,
  $selectedRelatedRecords,
  resetRelatedRecordsSelection,
  toggleRelatedRecordSelection
} from '~/features/records/stores/related-actionbar.ts'

function FieldHeadCell({
  field,
  ...props
}: THeadCellProps & {
  field: CollectProperty
}) {
  const sort = useStore($recordsOrderBy)
  const sortField = typeof sort === 'string' ? '__id' : (sort ?? {})[field.name]
  const sortActive =
    typeof sort === 'string'
      ? field.name === '__id'
      : !!(sort ?? {})[field.name]
  const sortDirection =
    typeof sort === 'string' ? sort : (sort ?? {})[field.name]

  return (
    <HeadCell
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

const RecordRowSkeleton = forwardRef<HTMLTableRowElement, { cols?: number }>(
  ({ cols = 1 }, ref) => {
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
  }
)
RecordRowSkeleton.displayName = 'RecordRowSkeleton'

function StickyHeaderWrapper({
  children,
  className,
  ...props
}: {
  children: (sticky: boolean) => ReactNode
  className?: string
}) {
  const el = useRef<HTMLDivElement>(null)
  const startBufferEl = useRef<HTMLDivElement>(null)
  const endBufferEl = useRef<HTMLDivElement>(null)

  const [sticky, setSticky] = useState(false)

  useEffect(() => {
    const wrapper = el.current

    if (wrapper) {
      const show = () => {
        if (!wrapper || wrapper.querySelector('[data-clone="true"]')) {
          return
        }

        setSticky(true)
      }

      const hide = () => {
        if (!wrapper) {
          return
        }

        setSticky(false)
      }

      if (startBufferEl.current && endBufferEl.current) {
        const thead = wrapper.querySelectorAll('thead')
        const rows = wrapper.querySelectorAll('tr')
        const theadHeight =
          ((thead && thead[0].getBoundingClientRect()) || {}).height || 0
        const lastRowHeight =
          ((rows && rows[rows.length - 1].getBoundingClientRect()) || {})
            .height || 0
        endBufferEl.current.style.top = `-${theadHeight + lastRowHeight / 2}px`
        const states = new Map()
        const observer = new IntersectionObserver(
          (entries) => {
            entries.forEach((e) => {
              states.set(e.target, e.boundingClientRect)
            })
            const { top } = states.get(startBufferEl.current) || {}
            const { top: bottom } = states.get(endBufferEl.current) || {}
            if (top < 0 && bottom > 0) {
              show()
            } else {
              hide()
            }
          },
          {
            threshold: [0]
          }
        )
        observer.observe(startBufferEl.current)
        observer.observe(endBufferEl.current)
      }
    }
  }, [])

  const syncStickyHeaderScroll: UIEventHandler<HTMLDivElement> = (event) => {
    // sync scroll
    const clonedHeader = (
      event.target as HTMLElement
    ).querySelector<HTMLElement>('[data-clone="true"]')

    if (!clonedHeader) {
      return
    }

    clonedHeader.style.transform = `translate3d(-${event.currentTarget?.scrollLeft}px,0,0)`
  }

  return (
    <div
      {...props}
      className={cn('overflow-auto', className)}
      onScroll={syncStickyHeaderScroll}
      ref={el}
    >
      <div ref={startBufferEl} />
      {children(sticky)}
      <div className="relative -z-10" ref={endBufferEl} />
    </div>
  )
}

const StickyTableHead = ({
  children,
  sticky
}: {
  children: ReactElement
  sticky: boolean
}) => {
  const trRef = useRef<HTMLTableRowElement>(null)
  const cloneRef = useRef<HTMLTableRowElement>(null)

  useEffect(() => {
    const tr = trRef.current
    if (tr && sticky) {
      const dimensions = [...tr.querySelectorAll('th')].map((th) => {
        const { width, height } = th.getBoundingClientRect()

        return { width: `${width}px`, height: `${height}px` }
      })

      cloneRef.current?.querySelectorAll('th').forEach((th, index) => {
        th.style.setProperty('--width', dimensions[index].width)
        th.style.setProperty('--height', dimensions[index].height)
      })
    }
  }, [children, sticky])

  const clonedTrWithRef = cloneElement(children, { ref: trRef })

  const clone = cloneElement(children, {
    'data-clone': 'true',
    ref: cloneRef,
    className: cn(
      children.props.className,
      'fixed top-0 z-10 flex min-w-[100vw] bg-fill shadow-2xl shadow-fill [&>th]:h-[var(--height)] [&>th]:w-[var(--width)] border-b'
    )
  })

  return (
    <thead>
      {clonedTrWithRef}
      {sticky && clone}
    </thead>
  )
}

export function RecordsTable({
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
}: TPolymorphicComponentProps<
  'div',
  {
    fields?: CollectProperty[]
    loading?: boolean
    onRecordClick?: (record: CollectRecord) => void
    records?: CollectRecord[]
    view?: 'related' | 'main'
  } & PaginatorProps
>) {
  const tableRef = useRef<HTMLTableElement>(null)
  const hiddenFields = useStore($hiddenFields)

  const internalIdField: CollectProperty = {
    id: '__id',
    type: 'string',
    name: '__id'
  }

  const scrollToHead = () => {
    if (!tableRef.current) {
      return
    }
    if (isInViewport(tableRef.current)) {
      return
    }
    tableRef.current?.scrollIntoView()
  }

  const selectedRecords = useStore($selectedRecords)
  const hasSelection = useStore($hasRecordsSelection)
  const mixedSelection = useStore($mixedRecordsSelection)

  const selectedRelatedRecords = useStore($selectedRelatedRecords)
  const hasRelatedSelection = useStore($hasRelatedRecordsSelection)
  const mixedRelatedSelection = useStore($mixedRelatedRecordsSelection)

  const getSelectedState = (id: string) =>
    view === 'main'
      ? !mixedSelection || selectedRecords.includes(id)
      : !mixedRelatedSelection || selectedRelatedRecords.includes(id)

  const checked = view === 'main' ? hasSelection : hasRelatedSelection
  const mixed = view === 'main' ? mixedSelection : mixedRelatedSelection

  const handleOnCheckedChange = () => {
    if (view === 'main') {
      if (hasSelection) {
        if (mixedSelection) {
          $selectedRecords.set(records?.map((r) => r.__id) as string[])
        } else {
          resetRecordsSelection()
        }
      } else {
        $selectedRecords.set(records?.map((r) => r.__id) as string[])
      }
    }

    if (view === 'related') {
      if (hasRelatedSelection) {
        if (mixedRelatedSelection) {
          $selectedRelatedRecords.set(records?.map((r) => r.__id) as string[])
        } else {
          resetRelatedRecordsSelection()
        }
      } else {
        $selectedRelatedRecords.set(records?.map((r) => r.__id) as string[])
      }
    }
  }

  if ((!records || records.length < 1) && !loading) {
    return <NothingFound />
  }

  return (
    <>
      <StickyHeaderWrapper className={className} {...props}>
        {(sticky) => (
          <table className="w-full" ref={tableRef}>
            <StickyTableHead sticky={sticky}>
              <tr className="group border-b bg-fill shadow-[0_5px_10px_rgba(0,0,0,0.2)]">
                <HeadCell style={{ width: 44 }}>
                  <Checkbox
                    className={cn('hidden group-hover:block', {
                      '!block': checked
                    })}
                    onCheckedChange={handleOnCheckedChange}
                    checked={checked}
                    mixed={mixed}
                  />
                </HeadCell>
                {!hiddenFields.includes('__id') && (
                  <FieldHeadCell
                    field={internalIdField}
                    key={internalIdField.id}
                  />
                )}

                {fields
                  ? fields?.map((field) => (
                      <FieldHeadCell field={field} key={field.id} />
                    ))
                  : range(1).map((key) => (
                      <HeadCell key={key}>
                        <Skeleton enabled>
                          <PropertyName name={'Loading'} type="string" />
                        </Skeleton>
                      </HeadCell>
                    ))}
              </tr>
            </StickyTableHead>
            <tbody>
              {(!records || !fields) &&
                range(records?.length ?? limit).map((index) => (
                  <RecordRowSkeleton key={index} />
                ))}

              {fields &&
                records &&
                records?.map((record, index) => {
                  if (!record) {
                    return null
                  }

                  const selected = getSelectedState(record.__id)

                  return (
                    <TableRow
                      className={cn('group cursor-pointer transition-colors', {
                        'bg-fill2': index % 2 === 0
                      })}
                      onClick={
                        onRecordClick ? () => onRecordClick(record) : undefined
                      }
                      key={record.__id}
                      style={{ height: 52.5 }}
                      tabIndex={0}
                    >
                      <DataCell className="text-content-secondary">
                        <Checkbox
                          className={cn('hidden group-hover:block', {
                            '!block': selected
                          })}
                          onCheckedChange={() =>
                            view === 'main'
                              ? toggleRecordSelection({
                                  recordId: record.__id,
                                  selected
                                })
                              : toggleRelatedRecordSelection({
                                  recordId: record.__id,
                                  selected
                                })
                          }
                          checked={selected}
                          onClick={(e) => e.stopPropagation()}
                        />
                        <Database
                          className={cn('group-hover:hidden', {
                            '!hidden': selected
                          })}
                          size={16}
                        />
                      </DataCell>
                      {!hiddenFields.includes('__id') && (
                        <DataCell
                          onPointerEnter={handlePointerEnter({
                            property: {
                              name: '__id',
                              type: 'string',
                              value: record.__id
                            },
                            showOperations: false
                          })}
                          className="py-1"
                          key={`${record.__id}-${index}`}
                          onPointerLeave={handlePointerLeave}
                        >
                          <Skeleton enabled={loading}>
                            <Label>{record.__label}</Label>

                            <PropertyValue
                              className="text-content2"
                              type={'string'}
                              value={record.__id}
                            />
                          </Skeleton>
                        </DataCell>
                      )}

                      {/* account for missing values */}
                      {fields?.map((field) => {
                        const property = collectPropertiesFromRecord(
                          record as CollectRecord
                        )?.find(
                          (p) => p.type === field.type && p.name === field.name
                        )

                        if (!property) {
                          // value missing
                          return (
                            <DataCell
                              className="text-content3"
                              key={`${record.__id}-${field.id}-${field.name}`}
                            >
                              —
                            </DataCell>
                          )
                        }

                        return (
                          <DataCell
                            key={`${record.__id}-${property.name}`}
                            onPointerEnter={handlePointerEnter({ property })}
                            onPointerLeave={handlePointerLeave}
                          >
                            <Skeleton enabled={loading}>
                              <PropertyValue
                                type={property.type}
                                value={property.value}
                              />
                            </Skeleton>
                          </DataCell>
                        )
                      })}
                    </TableRow>
                  )
                })}
            </tbody>
          </table>
        )}
      </StickyHeaderWrapper>

      <RecordsBatchActionsBar view={view} />

      {records?.length ? (
        <Paginator
          className="sticky bottom-0 border-t bg-fill"
          limit={limit}
          onNext={composeEventHandlers(scrollToHead, onNext)}
          onPrev={composeEventHandlers(scrollToHead, onPrev)}
          skip={skip}
          total={total}
        />
      ) : null}
    </>
  )
}
