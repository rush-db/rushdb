import type { CollectProperty } from '@collect.so/javascript-sdk'

import { useStore } from '@nanostores/react'
import { Check, X, Settings } from 'lucide-react'

import { Button } from '~/elements/Button'
import { ButtonGroup } from '~/elements/ButtonGroup'
import { IconButton } from '~/elements/IconButton'
import { SearchSelect, SelectItem } from '~/elements/SearchSelect'
import { SelectLabels } from '~/features/labels/components/SelectLabels'
import { SearchBox } from '~/features/projects/components/SearchBox.tsx'
import {
  $activeLabels,
  $currentProjectFields,
  $currentProjectFilters,
  $currentProjectLabels,
  $recordView,
  removeFilter,
  resetFilters
} from '~/features/projects/stores/current-project'
import { PropertyName } from '~/features/properties/components/PropertyName'
import { ApiRecordsModal } from '~/features/records/components/ApiRecordsModal'
import { IngestRecordsModal } from '~/features/records/components/IngestRecordsModal'
import { FilterPopover } from '~/features/search/components/FilterPopover'
import { cn } from '~/lib/utils'

import {
  $hiddenFields,
  $toggleHiddenField,
  isFieldHidden
} from '../stores/hidden-fields'
import { SelectCombineFiltersMode } from './SelectCombineFiltersMode'
import { Divider } from '~/elements/Divider.tsx'
import { SwitchField } from '~/elements/Switch.tsx'

const useProjectFiltersState = () => {
  const activeFilters = useStore($currentProjectFilters)
  const filtersArray = Object.values(activeFilters).flat()

  return {
    activeFilters,
    filtersArray,
    hasAnyFiltersApplied: !!filtersArray.length
  }
}

function ResetFiltersButton() {
  const { hasAnyFiltersApplied } = useProjectFiltersState()

  if (hasAnyFiltersApplied) {
    return (
      <Button onClick={resetFilters} size="small" variant="outline">
        <X />
        Reset filters
      </Button>
    )
  }
  return null
}

function Filters() {
  const { filtersArray } = useProjectFiltersState()

  return (
    <>
      {filtersArray.map((filter) => (
        <FilterPopover
          filter={filter}
          key={`filter-${filter.filterId}`}
          onRemove={removeFilter}
        />
      ))}
    </>
  )
}

function HiddenFieldsSelector() {
  const { data: fields } = useStore($currentProjectFields)
  const hiddenFields = useStore($hiddenFields)

  const internalIdField: CollectProperty = {
    id: '__id',
    type: 'string',
    name: '__id'
  }
  const viewMode = useStore($recordView)

  const allFields = [internalIdField, ...(fields ?? [])]

  return (
    <ButtonGroup>
      <SearchSelect
        trigger={
          <IconButton
            size="small"
            variant="outline"
            aria-label="columns-visibility"
          >
            <Settings />
          </IconButton>
        }
      >
        {allFields?.map((field) => (
          <SelectItem
            closeOnSelect={false}
            key={field.id}
            onSelect={() => $toggleHiddenField(field.id)}
          >
            <span className="hidden">{field.id}</span>
            <PropertyName name={field.name} type={field.type} />
            <Check
              className={cn(
                'ml-auto text-content2',
                isFieldHidden(hiddenFields, field.id)
                  ? 'opacity-0'
                  : 'opacity-100'
              )}
            />
          </SelectItem>
        ))}
        <Divider />

        <SwitchField
          checked={viewMode === 'graph'}
          className="flex flex-row justify-between p-4"
          label="Graph view"
          onCheckedChange={(checked) => {
            $recordView.set(checked ? 'graph' : 'table')
          }}
        />
      </SearchSelect>
    </ButtonGroup>
  )
}

export function RecordsHeader() {
  const { data: labels, loading: loadingLabels } = useStore(
    $currentProjectLabels
  )
  const { hasAnyFiltersApplied } = useProjectFiltersState()

  const activeLabels = useStore($activeLabels)

  return (
    <header className="flex justify-between gap-x-3 gap-y-3 border-b p-5">
      <div className="flex flex-1 flex-wrap items-start gap-inherit">
        <SearchBox
          prefix={
            <SelectCombineFiltersMode className="-ml-3 rounded-none border-0 border-r" />
          }
          className="max-w-sm"
          size="small"
        />
        <SelectLabels
          activeLabels={activeLabels}
          labels={labels}
          loading={loadingLabels}
          onSelect={$activeLabels.set}
        />
        <Filters />
      </div>

      <div className="flex gap-inherit">
        <ResetFiltersButton />
        {hasAnyFiltersApplied && <Divider vertical />}

        <ApiRecordsModal />
        <IngestRecordsModal />
        <Divider vertical />
        <HiddenFieldsSelector />
      </div>
    </header>
  )
}
