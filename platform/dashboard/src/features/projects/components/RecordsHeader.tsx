import { useStore } from '@nanostores/react'
import { useIsFetching, useQueryClient } from '@tanstack/react-query'
import {
  Check,
  X,
  Columns3Cog,
  KeyRound,
  Lock,
  Play,
  Code2,
  LoaderCircle,
  RefreshCw,
  Rows3,
  Rows4,
  SlidersHorizontal,
  Sparkles,
  Brain
} from 'lucide-react'
import type { FormEvent } from 'react'
import { useEffect, useRef, useState } from 'react'

import { Button } from '~/elements/Button'
import { ButtonGroup } from '~/elements/ButtonGroup'
import { IconButton } from '~/elements/IconButton'
import { Input } from '~/elements/Input'
import { Label } from '~/elements/Label'
import { RadioGroup } from '~/elements/RadioGroup'
import { SearchSelect, SelectItem } from '~/elements/SearchSelect'
import type { EmbeddingIndex } from '~/features/indexes/types'
import { SelectLabels } from '~/features/labels/components/SelectLabels'
import { getLabelColor } from '~/features/labels'
import { SearchBox } from '~/features/projects/components/SearchBox.tsx'
import { SearchQueryModal } from '~/features/projects/components/SearchQueryModal'
import { ExportCsvButton } from '~/features/records/components/ExportCsvButton'
import {
  $activeLabels,
  $currentProjectRecordsSkip,
  $currentProjectFilters,
  $recordView,
  removeFilter,
  resetFilters
} from '~/features/projects/stores/current-project'
import {
  $aiSearchPrompt,
  $semanticSearchIndexId,
  $semanticSearchPrompt,
  $searchQueryModalOpen,
  $recordsSearchMode,
  resetAiSearch,
  type RecordsSearchMode
} from '~/features/projects/stores/records-search'
import { PropertyName } from '~/features/properties/components/PropertyName'
import { PropertyIconSquare } from '~/features/properties/components/PropertyTypeIcon'
import {
  useGenerateSearchQueryMutation,
  useProjectFieldsQuery,
  useProjectIndexesQuery,
  useProjectLabelsQuery
} from '~/features/projects/hooks/useProjectQueries'

import { FilterPopover } from '~/features/search/components/FilterPopover'
import { cn } from '~/lib/utils'

import { $currentProjectId } from '~/features/projects/stores/id'
import { $hiddenFields, $resetHiddenFields, $toggleHiddenField, isFieldHidden } from '../stores/hidden-fields'
import { $rowDensity, type RowDensity } from '../stores/row-density'
import { SelectCombineFiltersMode } from './SelectCombineFiltersMode'
import { Divider } from '~/elements/Divider.tsx'
import { SelectViewMode } from '~/features/projects/components/SelectViewMode.tsx'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { Tooltip } from '~/elements/Tooltip'

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
        Reset filters
        <X />
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
        <FilterPopover filter={filter} key={`filter-${filter.filterId}`} onRemove={removeFilter} />
      ))}
    </>
  )
}

function RowDensitySelector() {
  const density = useStore($rowDensity)
  const options: Array<{ value: RowDensity; label: string; icon: React.ReactNode }> = [
    { value: 'normal', label: 'Normal', icon: <Rows3 /> },
    { value: 'compact', label: 'Compact', icon: <Rows4 /> }
  ]

  return (
    <div className="flex gap-1 border-b p-2">
      {options.map(({ value, label, icon }) => (
        <Button
          className="flex-1 items-center justify-center"
          key={value}
          onClick={() => $rowDensity.set(value)}
          size="xsmall"
          variant={density === value ? 'secondary' : 'ghost'}
        >
          {icon}
          {label}
        </Button>
      ))}
    </div>
  )
}

function HiddenFieldsSelector() {
  const { data: fields } = useProjectFieldsQuery()
  const hiddenFields = useStore($hiddenFields)

  // __id is the record's primary key — always present, never hideable. Keep it out of
  // the toggleable set and self-heal any persisted state that hid it.
  const toggleableFields = fields ?? []
  const allVisible = toggleableFields.every((field) => !isFieldHidden(hiddenFields, field.id))

  useEffect(() => {
    if (hiddenFields.includes('__id')) {
      $hiddenFields.set(hiddenFields.filter((id) => id !== '__id'))
    }
  }, [hiddenFields])

  const toggleAll = () => {
    if (allVisible) {
      $hiddenFields.set(toggleableFields.map((field) => field.id))
    } else {
      $resetHiddenFields()
    }
  }

  return (
    <ButtonGroup>
      <SearchSelect
        header={<RowDensitySelector />}
        trigger={
          <IconButton size="small" variant="outline" aria-label="columns-visibility">
            <Columns3Cog />
          </IconButton>
        }
      >
        <SelectItem closeOnSelect={false} onSelect={toggleAll}>
          <span className="font-medium">{allVisible ? 'Deselect all' : 'Select all'}</span>
          <Check className={cn('ml-auto text-content2', allVisible ? 'opacity-100' : 'opacity-0')} />
        </SelectItem>
        <Divider />
        <SelectItem closeOnSelect={false}>
          <span className="hidden">__id</span>
          <PropertyIconSquare className="bg-secondary text-content2" size={16}>
            <KeyRound size={11} />
          </PropertyIconSquare>
          <span className="font-mono">__id</span>
          <Tooltip align="end" trigger={<Lock className="ml-auto size-3.5 shrink-0 text-content3" />}>
            <span className="max-w-[220px] text-sm">The primary key is always shown.</span>
          </Tooltip>
        </SelectItem>
        {toggleableFields.map((field) => (
          <SelectItem closeOnSelect={false} key={field.id} onSelect={() => $toggleHiddenField(field.id)}>
            <span className="hidden">{field.id}</span>
            <PropertyName name={field.name} type={field.type} />
            <Check
              className={cn(
                'ml-auto text-content2',
                isFieldHidden(hiddenFields, field.id) ? 'opacity-0' : 'opacity-100'
              )}
            />
          </SelectItem>
        ))}
        <Divider />
      </SearchSelect>
    </ButtonGroup>
  )
}

function RefreshButton() {
  const projectId = useStore($currentProjectId)
  const queryClient = useQueryClient()
  const fetching = useIsFetching({
    queryKey: projectId ? ['projects', projectId, 'records'] : ['projects']
  })
  const isRefreshing = fetching > 0

  const refresh = () => {
    if (!projectId) return
    queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'records'] })
    queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'fields'] })
  }

  return (
    <Tooltip
      trigger={
        <IconButton aria-label="refresh-records" onClick={refresh} size="small" variant="outline">
          <RefreshCw className={cn(isRefreshing && 'animate-spin')} />
        </IconButton>
      }
    >
      <div className="flex items-center gap-1 text-2xs text-content uppercase">Refresh</div>
    </Tooltip>
  )
}

function SearchModeToggle() {
  const mode = useStore($recordsSearchMode)
  const options: Array<{ value: RecordsSearchMode; label: string; icon: React.ReactNode }> = [
    {
      value: 'ai',
      label: 'Smart',
      icon: (
        <>
          <Sparkles />
          Smart
        </>
      )
    },
    {
      value: 'manual',
      label: 'Builder',
      icon: (
        <>
          <SlidersHorizontal />
          Builder
        </>
      )
    },
    {
      value: 'semantic',
      label: 'Semantic',
      icon: (
        <>
          <Brain />
          Semantic
        </>
      )
    }
  ]

  return (
    <RadioGroup
      className="w-fit"
      itemWidth={96}
      onChange={$recordsSearchMode.set}
      options={options}
      size="small"
      useDefaultButton
      value={mode}
    />
  )
}

const formatIndexSource = (sourceType?: EmbeddingIndex['sourceType']) =>
  sourceType === 'external' ? 'Custom vectors' : 'Managed'

const getIndexSourceTag = (sourceType?: EmbeddingIndex['sourceType']) =>
  sourceType === 'external' ? 'C' : 'M'

function SemanticIndexSummary({
  className,
  index,
  labelNames,
  selected = false
}: {
  className?: string
  index: EmbeddingIndex
  labelNames: string[]
  selected?: boolean
}) {
  const labelVariant = getLabelColor(index.label, labelNames.indexOf(index.label))
  const similarity = index.similarityFunction ?? 'cosine'

  if (selected) {
    return (
      <div
        className={cn('flex h-full max-w-full min-w-0 items-center gap-2', className)}
        title={`${index.label}.${index.propertyName} · ${formatIndexSource(index.sourceType)} · ${index.dimensions}d · ${similarity}`}
      >
        <Label className="shrink-0" variant={labelVariant}>
          {index.label}
        </Label>
        <span className="min-w-0 truncate text-sm font-medium text-content">{index.propertyName}</span>
        <span className="ml-auto inline-grid h-5 w-5 shrink-0 place-items-center rounded-sm bg-secondary font-mono text-xs text-content2">
          {getIndexSourceTag(index.sourceType)}
        </span>
      </div>
    )
  }

  return (
    <div className={cn('flex min-w-0 flex-col gap-1', className)}>
      <div className="flex min-w-0 items-center gap-2">
        <Label className="shrink-0" variant={labelVariant}>
          {index.label}
        </Label>
        <span className="min-w-0 truncate text-sm font-medium text-content">{index.propertyName}</span>
      </div>
      <div className="flex min-w-0 items-center gap-x-2 text-xs text-content2">
        <span className="inline-grid h-5 w-5 shrink-0 place-items-center rounded-sm bg-secondary font-mono text-content2">
          {getIndexSourceTag(index.sourceType)}
        </span>
        <span className="truncate">
          <span>{formatIndexSource(index.sourceType)}</span>
        </span>
        <span>{index.dimensions}d</span>
        <span>{similarity}</span>
      </div>
    </div>
  )
}

function SemanticIndexSelect({ labels }: { labels?: Record<string, number> }) {
  const selectedIndexId = useStore($semanticSearchIndexId)
  const [open, setOpen] = useState(false)
  const { data: indexes, isPending } = useProjectIndexesQuery({ enabled: open })
  const readyIndexes = indexes?.filter((index) => index.enabled && index.status === 'ready') ?? []
  const selectedIndex = readyIndexes.find((index) => index.id === selectedIndexId)
  const labelNames = Object.keys(labels ?? {})

  return (
    <SearchSelect
      onOpenChange={setOpen}
      open={open}
      trigger={
        <Button
          className="h-9 w-72 max-w-72 justify-start overflow-hidden"
          disabled={open && isPending}
          size="small"
          variant="outline"
        >
          {selectedIndex ?
            <SemanticIndexSummary index={selectedIndex} labelNames={labelNames} selected />
          : <>
              <Brain />
              <span className="min-w-0 truncate">Select index</span>
            </>
          }
        </Button>
      }
    >
      {readyIndexes.map((index) => (
        <SelectItem
          className="h-auto items-start py-2"
          key={index.id}
          onSelect={() => $semanticSearchIndexId.set(index.id)}
        >
          <SemanticIndexSummary className="flex-1" index={index} labelNames={labelNames} />
          {selectedIndexId === index.id && <Check className="ml-auto text-accent" />}
        </SelectItem>
      ))}
      {open && isPending && <SelectItem disabled>Loading semantic indexes...</SelectItem>}
      {!isPending && readyIndexes.length === 0 && (
        <SelectItem className="h-auto py-2" disabled>
          <div className="flex flex-col gap-1">
            <span>No ready semantic indexes</span>
            <span className="text-xs text-content2">
              Create a semantic index from the Semantic Indexes page.
            </span>
          </div>
        </SelectItem>
      )}
    </SearchSelect>
  )
}

function SemanticSearchSurface({ labels }: { labels?: Record<string, number> }) {
  const prompt = useStore($semanticSearchPrompt)
  const indexId = useStore($semanticSearchIndexId)
  const [draftPrompt, setDraftPrompt] = useState(prompt)
  const debounceTimeoutRef = useRef<number | null>(null)
  const disabled = !indexId

  useEffect(() => {
    setDraftPrompt(prompt)
  }, [prompt])

  useEffect(() => {
    if (debounceTimeoutRef.current != null) {
      window.clearTimeout(debounceTimeoutRef.current)
      debounceTimeoutRef.current = null
    }

    if (disabled || draftPrompt === prompt) {
      return
    }

    debounceTimeoutRef.current = window.setTimeout(() => {
      $semanticSearchPrompt.set(draftPrompt)
      $currentProjectRecordsSkip.set(0)
      debounceTimeoutRef.current = null
    }, 400)

    return () => {
      if (debounceTimeoutRef.current != null) {
        window.clearTimeout(debounceTimeoutRef.current)
        debounceTimeoutRef.current = null
      }
    }
  }, [disabled, draftPrompt, prompt])

  return (
    <div className="flex min-w-0 flex-wrap items-center gap-3">
      <SemanticIndexSelect labels={labels} />
      <Input
        className="w-full max-w-md"
        disabled={disabled}
        onChange={(event) => setDraftPrompt(event.target.value)}
        placeholder={disabled ? 'Select an index before searching' : 'Search by meaning'}
        prefix={<Sparkles className="text-accent" />}
        size="small"
        type="search"
        value={draftPrompt}
      />
    </div>
  )
}

function AiSearchBox() {
  const prompt = useStore($aiSearchPrompt)
  const { data: platformSettings, isPending: loadingSettings } = usePlatformSettings()
  const generateSearchQuery = useGenerateSearchQueryMutation()
  const disabled = loadingSettings || platformSettings?.llmEnabled !== true || generateSearchQuery.isPending

  const submit = (event: FormEvent) => {
    event.preventDefault()
    if (disabled || !prompt.trim()) {
      return
    }
    generateSearchQuery.mutate({ prompt })
  }

  return (
    <form className="flex min-w-0 flex-wrap items-center gap-3" onSubmit={submit}>
      <Input
        className="w-full max-w-md"
        disabled={disabled}
        onChange={(event) => {
          const value = event.target.value
          // Clearing the input (incl. the native [X]) restores the default results view.
          if (!value.trim()) {
            resetAiSearch()
            return
          }
          $aiSearchPrompt.set(value)
        }}
        placeholder={
          platformSettings?.llmEnabled === false ?
            'AI search is not configured'
          : 'Describe what you’re looking for'
        }
        prefix={<Sparkles className="text-accent" />}
        size="small"
        type="search"
        value={prompt}
      />
      <Tooltip
        trigger={
          <Button
            aria-label="run-ai-search"
            disabled={disabled || !prompt.trim()}
            size="small"
            type="submit"
            variant="primary"
          >
            {generateSearchQuery.isPending ?
              <LoaderCircle className="animate-spin" />
            : <Play />}
            Run
          </Button>
        }
      >
        <div className="flex items-center gap-1 text-2xs text-content uppercase">Run AI search</div>
      </Tooltip>
    </form>
  )
}

function SearchQueryButton({ label = false }: { label?: boolean }) {
  const trigger =
    label ?
      <Button
        aria-label="raw-search-query"
        onClick={() => $searchQueryModalOpen.set(true)}
        size="small"
        variant="outline"
      >
        <Code2 />
        View Query
      </Button>
    : <IconButton
        aria-label="raw-search-query"
        onClick={() => $searchQueryModalOpen.set(true)}
        size="small"
        variant="outline"
      >
        <Code2 />
      </IconButton>

  return (
    <Tooltip trigger={trigger}>
      <div className="flex items-center gap-1 text-2xs text-content uppercase">View query</div>
    </Tooltip>
  )
}

function BuilderSearchSurface({
  activeLabels,
  labels,
  loadingLabels
}: {
  activeLabels: string[]
  labels?: Record<string, number>
  loadingLabels: boolean
}) {
  return (
    <div className="flex min-w-0 flex-wrap items-center gap-3">
      <SearchBox
        data-tour="records-table-search-input"
        prefix={<SelectCombineFiltersMode className="-ml-3 rounded-none border-0 border-r" />}
        className="w-full max-w-sm"
        size="small"
      />
      <SelectLabels
        activeLabels={activeLabels}
        labels={labels}
        loading={loadingLabels}
        onSelect={$activeLabels.set}
      />
    </div>
  )
}

export function RecordsHeader() {
  const { data: labels, isPending: loadingLabels } = useProjectLabelsQuery()
  const { hasAnyFiltersApplied } = useProjectFiltersState()
  const view = useStore($recordView)
  const mode = useStore($recordsSearchMode)

  const activeLabels = useStore($activeLabels)

  return (
    <header className="flex flex-col gap-3 border-b p-5">
      <div className="flex items-center justify-between gap-3">
        <SearchModeToggle />
        <SearchQueryModal />
        <div className="flex gap-3">
          <ExportCsvButton />
          <SelectViewMode />
        </div>
      </div>

      <div className="flex min-w-0 items-center gap-3">
        <div className="min-w-0 flex-1">
          {mode === 'ai' ?
            <AiSearchBox />
          : mode === 'semantic' ?
            <SemanticSearchSurface labels={labels} />
          : <BuilderSearchSurface activeLabels={activeLabels} labels={labels} loadingLabels={loadingLabels} />
          }
        </div>
        <SearchQueryButton label />
        <RefreshButton />
        {view === 'table' && <HiddenFieldsSelector />}
      </div>

      {hasAnyFiltersApplied && (
        <div className="flex flex-wrap items-center gap-3">
          <Filters />
          <div className="ml-auto">
            <ResetFiltersButton />
          </div>
        </div>
      )}
    </header>
  )
}
