import type { Property, PropertyType, PropertyValue } from '@rushdb/javascript-sdk'
import type { ComponentPropsWithoutRef, ReactNode } from 'react'

import { useStore } from '@nanostores/react'
import { action, atom, computed, map } from 'nanostores'
import { useEffect, useMemo, useRef } from 'react'
import useVirtual from 'react-cool-virtual'

import type { DateRange } from '~/elements/Calendar.tsx'
import type { SearchOperations } from '~/features/search/constants.ts'

import {
  Combobox,
  ComboboxEmpty,
  ComboboxGroup,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
  ComboboxPopover,
  ComboboxTitle,
  SearchItem
} from '~/elements/Combobox.tsx'
import { AngledSeparator } from '~/elements/Divider.tsx'
import { Kbd } from '~/elements/Kbd.tsx'
import { PropertyName } from '~/features/properties/components/PropertyName.tsx'
import { PropertyTypeIcon } from '~/features/properties/components/PropertyTypeIcon.tsx'
import { formatPropertyValue } from '~/features/properties/utils.ts'
import { SearchOperationIcon } from '~/features/search/components/SearchOperationIcon.tsx'
import { operatorOptions } from '~/features/search/constants.ts'
import { type AnySearchOperation, isViableSearchOperation } from '~/features/search/types.ts'
import { useHotkeys } from '~/hooks/useHotkeys.ts'
import { api } from '~/lib/api.ts'
import { createAsyncStore } from '~/lib/fetcher.ts'
import { isInViewport, normalizeString } from '~/lib/utils.ts'

import {
  $activeLabels,
  $currentProjectFields,
  $currentProjectFilters,
  addFilter
} from '../stores/current-project.ts'
import { convertToSearchQuery, filterToSearchOperation } from '~/features/projects/utils.ts'

const ICON_SIZE = 12

const ITEM_HEIGHT = 44

// number of chars to perform value matching
const MAX_SEARCH_CHARS = 200

export const $open = atom<boolean>(false)

const $recordQuery = atom<string>('')

// pages
enum BoxPages {
  // ChooseMinMax = 'ChooseMinMax',
  // DateRange = 'DateRange',
  Operators = 'Operators',
  // SetMax = 'SetMax',
  // SetMin = 'SetMin',
  Values = 'Values'
}
const $pages = atom<BoxPages[]>([])
const $page = computed($pages, (pages) => pages[pages.length - 1])
const goPageBack = action($pages, 'goPageBack', (store) => {
  return store.set(store.get().slice(0, -1))
})
const goPageForward = action($pages, 'goPageForward', (store, page: BoxPages) => {
  return store.set([...store.get(), page])
})

// current
const $currentField = atom<Property | undefined>(undefined)
const $currentFieldValues = createAsyncStore({
  key: '$fieldValues',
  deps: [$currentField],
  async fetcher(init) {
    const fieldId = $currentField.get()?.id

    if (!fieldId) {
      return
    }

    const labels = $activeLabels.get()
    let properties = $currentProjectFilters.get().map(filterToSearchOperation)

    return await api.properties.values({
      init,
      id: fieldId,
      searchQuery: {
        labels,
        where: convertToSearchQuery(properties)
      }
    })

    // as unknown as Array<
    //    Property &
    //    PropertyValuesData & { values: PropertySingleValue }
    //  >
  }
})
const $currentOperator = atom<SearchOperations | undefined>(undefined)
const $currentMin = atom<number | undefined>()
const $currentMax = atom<number | undefined>()
const initialDateRange = {
  from: undefined,
  to: undefined
}
const $currentDateRange = map<DateRange>(initialDateRange)

const resetPages = action($pages, 'resetPages', (store) => {
  $currentField.set(undefined)
  $currentOperator.set(undefined)
  $currentMin.set(undefined)
  $currentMax.set(undefined)
  $currentDateRange.set({ from: undefined, to: undefined })
  return store.set([])
})

$page.subscribe((page) => {
  $recordQuery.set('')
  if (page === undefined) {
    resetPages()
  }
})

$open.subscribe(() => {
  resetPages()
})

const select = (operation: AnySearchOperation) => {
  addFilter(operation)
  $open.set(false)
  $recordQuery.set('')
  $currentMin.set(undefined)
  $currentMax.set(undefined)
  $currentField.set(undefined)
  $currentOperator.set(undefined)
  resetPages()
}

const selectMin = (min?: number) => {
  $currentMin.set(min)
  goPageBack()
}

const selectMax = (max?: number) => {
  $currentMax.set(max)
  goPageBack()
}

function CurrentValueSuggestion({
  onSelect,
  disabled,
  children
}: {
  children?: ReactNode
  disabled?: boolean
  onSelect: (value: PropertyValue) => void
}) {
  const { data: fieldValues } = useStore($currentFieldValues)
  const query = useStore($recordQuery)

  return (
    <SearchItem
      onSelect={() => {
        onSelect(query)
      }}
      disabled={disabled || query.length < 1}
      hasMatch={fieldValues?.values?.some((value) => value == query)}
      placeholder="Start typing..."
    >
      {children || query}
    </SearchItem>
  )
}

/** filter operator options by field type */
const operatorByFieldType = (fieldType: PropertyType) => (operatorOption: (typeof operatorOptions)[number]) =>
  isViableSearchOperation({
    propertyType: fieldType,
    searchOperation: operatorOption.value
  })

function OperationItem({ start, end }: { end?: ReactNode; start?: ReactNode }) {
  return (
    <div className="flex w-full flex-col">
      <span>{start}</span>
      <span className="text-content2 hidden truncate text-xs sm:block">{end}</span>
    </div>
  )
}

// function ChooseMinMaxPage() {
//   const currentField = useStore($currentField)
//   const currentOperator = useStore($currentOperator)
//
//   const currentMin = useStore($currentMin)
//   const currentMax = useStore($currentMax)
//
//   return (
//     <>
//       <ComboboxItem onSelect={() => goPageForward(BoxPages.SetMin)}>
//         <Edit />
//         <OperationItem
//           start={
//             currentMin ? `Edit min value (${currentMin})` : 'Set min value'
//           }
//         />
//       </ComboboxItem>
//       <ComboboxItem onSelect={() => goPageForward(BoxPages.SetMax)}>
//         <Edit />
//         <OperationItem
//           start={
//             currentMax ? `Edit max value (${currentMax})` : 'Set max value'
//           }
//         />
//       </ComboboxItem>
//       {(currentMin || currentMax) && (
//         <ComboboxItem
//           onSelect={() =>
//             select({
//               operation: currentOperator,
//               // min: currentMin,
//               // max: currentMax,
//               value: currentMin,
//               name: currentField!.name
//             } as AnySearchOperation)
//           }
//         >
//           <PlusCircle />
//           Add filter (min: {currentMin ?? '∞'}, max: {currentMax ?? '∞'})
//         </ComboboxItem>
//       )}
//     </>
//   )
// }

// function SetMaxPage() {
//   const currentField = useStore($currentField)
//   const currentMin = useStore($currentMin)
//   const currentMax = useStore($currentMax)
//   const query = useStore($recordQuery)
//   const { data: fieldValues } = useStore($currentFieldValues)
//
//   const min = currentMin ?? -Infinity
//
//   const validQuery = !Number.isNaN(query) && Number(query) >= min
//
//   return (
//     <>
//       <CurrentValueSuggestion
//         disabled={!validQuery}
//         onSelect={() => selectMax(Number(query))}
//       >
//         {!validQuery &&
//           `Must be a number and be >= ${currentMin ?? 'minimal value'}`}
//       </CurrentValueSuggestion>
//
//       {currentMax && (
//         <ComboboxItem onSelect={() => selectMax(undefined)}>
//           <RotateCcw />
//           Reset
//         </ComboboxItem>
//       )}
//
//       {fieldValues?.values
//         ?.filter((value) => Number(value) >= min)
//         .map((value) => (
//           <ComboboxItem
//             key={`value-${value}`}
//             onSelect={(value) => selectMax(Number(value))}
//             value={String(value)}
//           >
//             <PropertyTypeIcon type={currentField!.type} />
//             <span className="truncate">{value as PrimitiveValue}</span>
//           </ComboboxItem>
//         ))}
//     </>
//   )
// }
//
// function SetDateRange() {
//   const currentField = useStore($currentField)
//   const currentRange = useStore($currentDateRange)
//
//   useHotkeys({
//     enter: () => {
//       if (!currentRange.from) {
//         return
//       }
//       // max: currentRange?.to ? formatIso(currentRange.to) : undefined,
//
//       select({
//         operation: SearchOperations.GreaterOrEqual,
//         value: formatIso(currentRange?.from),
//         name: currentField!.name
//       })
//     }
//   })
//
//   return (
//     <>
//       {/* <Input placeholder="From" value={fom currentRange.from} /> */}
//       <Calendar
//         onSelect={(range) => {
//           $currentDateRange.set(range ?? initialDateRange)
//         }}
//         className="py-1.5"
//         // defaultMonth={date?.from}
//         mode="range"
//         numberOfMonths={2}
//         selected={currentRange}
//       />
//     </>
//   )
// }
//
// function SetMinPage() {
//   const currentField = useStore($currentField)
//   // const currentOperator = useStore($currentOperator)
//   const { data: fieldValues } = useStore($currentFieldValues)
//   const currentMin = useStore($currentMin)
//   const currentMax = useStore($currentMax)
//   const query = useStore($recordQuery)
//
//   const max = currentMin ?? Infinity
//
//   const isValid = (value: any) => !Number.isNaN(value) && Number(value) <= max
//
//   const validQuery = isValid(query)
//
//   return (
//     <>
//       <CurrentValueSuggestion
//         disabled={!validQuery}
//         onSelect={() => selectMin(Number(query))}
//       >
//         {!validQuery &&
//           `Must be a number and be <= ${currentMax ?? 'max value'}`}
//       </CurrentValueSuggestion>
//
//       {currentMin && (
//         <ComboboxItem onSelect={() => selectMin(undefined)}>
//           <RotateCcw />
//           Reset
//         </ComboboxItem>
//       )}
//
//       {fieldValues?.values?.filter(isValid).map((value) => (
//         <ComboboxItem
//           key={`value-${value}`}
//           onSelect={(value) => selectMin(Number(value))}
//           value={String(value)}
//         >
//           <PropertyTypeIcon type={currentField!.type} />
//           {value as PrimitiveValue}
//         </ComboboxItem>
//       ))}
//     </>
//   )
// }

export function SearchBox({
  size,
  prefix,
  ...props
}: Omit<ComponentPropsWithoutRef<typeof Combobox>, 'prefix' | 'size'> &
  Pick<ComponentPropsWithoutRef<typeof ComboboxInput>, 'prefix' | 'size'>) {
  const query = useStore($recordQuery)
  const page = useStore($page)
  const currentField = useStore($currentField)
  const { data: fieldValues } = useStore($currentFieldValues)
  const currentOperator = useStore($currentOperator)

  const open = useStore($open)

  // props
  const { data: fields = [] } = useStore($currentProjectFields)

  const filteredValues = useMemo(
    () =>
      fieldValues?.values?.filter((value) => normalizeString(String(value)).includes(normalizeString(query))),
    [query, fieldValues]
  )

  const itemsCount = filteredValues?.length ?? 0

  const { outerRef, innerRef, items } = useVirtual<HTMLDivElement>({
    itemCount: itemsCount,
    itemSize: ITEM_HEIGHT
    // resetScroll: true
  })

  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    const target = inputRef?.current

    if (open && target) {
      target.focus()

      if (!isInViewport(target)) target.scrollIntoView()
    }
  }, [open])

  useHotkeys({
    'meta + k': () => $open.set(true),
    'ctrl + k': () => $open.set(true)
  })

  return (
    <Combobox
      {...props}
      onKeyDown={(e) => {
        // Escape goes to previous page
        // Backspace goes to previous page when query is empty
        if ((e.key === 'Escape' && page) || (e.key === 'Backspace' && !query)) {
          e.preventDefault()
          // setPages((pages) => pages.slice(0, -1))
          goPageBack()
          return
        }
      }}
      onOpenChange={$open.set}
      open={open}
      shouldFilter={page !== BoxPages.Values}
    >
      <ComboboxInput
        suffix={
          <div className="px-inherit flex gap-1">
            <Kbd code="Meta" pressed />
            <Kbd code="K" />
          </div>
        }
        onValueChange={$recordQuery.set}
        placeholder="Add filter"
        prefix={prefix}
        ref={inputRef}
        showSearchIcon={false}
        size={size}
        value={query}
      />

      <ComboboxPopover>
        <div>
          <ComboboxTitle className="sticky top-0">
            {!page && 'properties'}

            {page === BoxPages.Operators && (
              <div className="flex items-center">
                <PropertyName iconSize={ICON_SIZE} name={currentField!.name} type={currentField!.type} />
                <AngledSeparator size={ICON_SIZE} />
              </div>
            )}

            {/*{page === BoxPages.ChooseMinMax && (*/}
            {/*  <div className="flex items-center">*/}
            {/*    <PropertyName*/}
            {/*      iconSize={ICON_SIZE}*/}
            {/*      name={currentField!.name}*/}
            {/*      type={currentField!.type}*/}
            {/*    />*/}
            {/*    <AngledSeparator size={ICON_SIZE} />*/}
            {/*    {currentOperator}*/}
            {/*    <AngledSeparator size={ICON_SIZE} />*/}
            {/*  </div>*/}
            {/*)}*/}

            {/*{page === BoxPages.SetMax && (*/}
            {/*  <div className="flex items-center">*/}
            {/*    <PropertyName*/}
            {/*      iconSize={ICON_SIZE}*/}
            {/*      name={currentField!.name}*/}
            {/*      type={currentField!.type}*/}
            {/*    />*/}
            {/*    <AngledSeparator size={ICON_SIZE} />*/}
            {/*    {currentOperator}*/}
            {/*    <AngledSeparator size={ICON_SIZE} />*/}
            {/*    Set max*/}
            {/*  </div>*/}
            {/*)}*/}

            {/*{page === BoxPages.SetMin && (*/}
            {/*  <div className="flex items-center">*/}
            {/*    <PropertyName*/}
            {/*      iconSize={ICON_SIZE}*/}
            {/*      name={currentField!.name}*/}
            {/*      type={currentField!.type}*/}
            {/*    />*/}
            {/*    <AngledSeparator size={ICON_SIZE} />*/}
            {/*    {currentOperator}*/}
            {/*    <AngledSeparator size={ICON_SIZE} />*/}
            {/*    Set min*/}
            {/*  </div>*/}
            {/*)}*/}

            {page === BoxPages.Values && (
              <div className="flex items-center">
                <PropertyName iconSize={ICON_SIZE} name={currentField!.name} type={currentField!.type} />
                <AngledSeparator size={ICON_SIZE} />
                {currentOperator}
              </div>
            )}

            {/*{page === BoxPages.DateRange && (*/}
            {/*  <div className="flex items-center">*/}
            {/*    <PropertyName*/}
            {/*      iconSize={ICON_SIZE}*/}
            {/*      name={currentField!.name}*/}
            {/*      type={currentField!.type}*/}
            {/*    />*/}
            {/*    <AngledSeparator size={ICON_SIZE} />*/}
            {/*    {currentOperator}*/}
            {/*    <AngledSeparator size={ICON_SIZE} />*/}
            {/*  </div>*/}
            {/*)}*/}
          </ComboboxTitle>

          <ComboboxList ref={outerRef}>
            {!page && (
              <>
                {fields.map((field) => (
                  <ComboboxItem
                    onSelect={() => {
                      $currentField.set(field)
                      goPageForward(BoxPages.Operators)
                    }}
                    key={`${field.id}-${field.name}`}
                    value={`${field.id}-${field.name}`}
                  >
                    <span className="hidden">{field.id}</span>
                    <PropertyTypeIcon type={field.type} />
                    {field.name}
                  </ComboboxItem>
                ))}

                {fields.length === 0 && <ComboboxItem disabled>Nothing found</ComboboxItem>}
                <ComboboxEmpty>No property match...</ComboboxEmpty>
              </>
            )}
            {page === BoxPages.Operators && (
              <>
                {operatorOptions.filter(operatorByFieldType(currentField!.type)).map((operation) => (
                  <ComboboxItem
                    onSelect={() => {
                      $currentOperator.set(operation.value)

                      // switch (operation.value) {
                      //   case SearchOperations.Range:
                      //   case SearchOperations.ExcludeRange:
                      //     const nextPage =
                      //       currentField?.type === 'datetime'
                      //         ? BoxPages.DateRange
                      //         : BoxPages.ChooseMinMax
                      //     return goPageForward(nextPage)
                      //   default:
                      goPageForward(BoxPages.Values)
                      // }
                    }}
                    key={operation.value}
                    value={operation.label}
                  >
                    <SearchOperationIcon operation={operation.value} variant="filled" />
                    <OperationItem end={operation.description} start={operation.label} />
                  </ComboboxItem>
                ))}

                <ComboboxEmpty>No operator match...</ComboboxEmpty>
              </>
            )}
            {/*{page === BoxPages.ChooseMinMax && <ChooseMinMaxPage />}*/}
            {/*{page === BoxPages.SetMin && <SetMinPage />}*/}
            {/*{page === BoxPages.SetMax && <SetMaxPage />}*/}
            {/*{page === BoxPages.DateRange && <SetDateRange />}*/}
            {page === BoxPages.Values && (
              <>
                <CurrentValueSuggestion
                  onSelect={() =>
                    select({
                      name: currentField!.name, // TODO: remove name
                      operation: $currentOperator.get()!,
                      value: query
                    } as AnySearchOperation)
                  }
                />

                <ComboboxGroup>
                  {items.map(({ index, size }) => {
                    const value = filteredValues?.[index]

                    if (typeof value === 'undefined') {
                      return null
                    }

                    return (
                      <ComboboxItem
                        onSelect={() => {
                          select({
                            name: currentField!.name, // TODO: remove name
                            operation: $currentOperator.get()!,
                            value
                          } as AnySearchOperation)
                        }}
                        key={`${currentField!.name}-value-${value}`}
                        style={{ height: size }}
                        value={value?.toString().slice(0, MAX_SEARCH_CHARS)}
                      >
                        <span className="hidden">{currentField!.id}</span>
                        <PropertyTypeIcon type={currentField!.type} />

                        <span className="truncate">
                          {formatPropertyValue({
                            value,
                            type: currentField!.type
                          })}
                        </span>
                      </ComboboxItem>
                    )
                  })}
                </ComboboxGroup>
              </>
            )}
          </ComboboxList>

          <div className="bg-fill sticky bottom-0 flex gap-3 border-t px-3 py-1.5 shadow">
            {page && (
              <Kbd code="Backspace" onClick={goPageBack}>
                Back
              </Kbd>
            )}

            <Kbd code="ESC">Close</Kbd>

            <Kbd code={'ArrowDownUp'}>Navigate</Kbd>

            {/*{page === BoxPages.DateRange && <Kbd code="Enter">Confirm</Kbd>}*/}
          </div>
        </div>
      </ComboboxPopover>
    </Combobox>
  )
}
