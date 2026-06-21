import type { Property } from '@rushdb/javascript-sdk'

import { useStore } from '@nanostores/react'
import {
  Check,
  X,
  Columns3Cog,
  Play,
  Code2,
  LoaderCircle,
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
  type RecordsSearchMode
} from '~/features/projects/stores/records-search'
import { PropertyName } from '~/features/properties/components/PropertyName'
import {
  useGenerateSearchQueryMutation,
  useProjectFieldsQuery,
  useProjectIndexesQuery,
  useProjectLabelsQuery
} from '~/features/projects/hooks/useProjectQueries'

import { FilterPopover } from '~/features/search/components/FilterPopover'
import { cn } from '~/lib/utils'

import { $hiddenFields, $toggleHiddenField, isFieldHidden } from '../stores/hidden-fields'
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

function HiddenFieldsSelector() {
  const { data: fields } = useProjectFieldsQuery()
  const hiddenFields = useStore($hiddenFields)

  const internalIdField: Property = {
    id: '__id',
    type: 'string',
    name: '__id'
  }

  const allFields = [internalIdField, ...(fields ?? [])]

  return (
    <ButtonGroup>
      <SearchSelect
        trigger={
          <IconButton size="small" variant="outline" aria-label="columns-visibility">
            <Columns3Cog />
          </IconButton>
        }
      >
        {allFields?.map((field) => (
          <SelectItem closeOnSelect={false} key={field.id} onSelect={() => $toggleHiddenField(field.id)}>
            <span className="hidden">{field.id}</span>
            <PropertyName name={field.name} type={field.type} />
            <Check
              className={cn(
                'text-content2 ml-auto',
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
        className={cn('flex h-full min-w-0 max-w-full items-center gap-2', className)}
        title={`${index.label}.${index.propertyName} · ${formatIndexSource(index.sourceType)} · ${index.dimensions}d · ${similarity}`}
      >
        <Label className="shrink-0" variant={labelVariant}>
          {index.label}
        </Label>
        <span className="text-content min-w-0 truncate text-sm font-medium">{index.propertyName}</span>
        <span className="bg-secondary text-content2 ml-auto inline-grid h-5 w-5 shrink-0 place-items-center rounded-sm font-mono text-xs">
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
        <span className="text-content min-w-0 truncate text-sm font-medium">{index.propertyName}</span>
      </div>
      <div className="text-content2 flex min-w-0 items-center gap-x-2 text-xs">
        <span className="bg-secondary text-content2 inline-grid h-5 w-5 shrink-0 place-items-center rounded-sm font-mono">
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
          {selectedIndexId === index.id && <Check className="text-accent ml-auto" />}
        </SelectItem>
      ))}
      {open && isPending && <SelectItem disabled>Loading semantic indexes...</SelectItem>}
      {!isPending && readyIndexes.length === 0 && (
        <SelectItem className="h-auto py-2" disabled>
          <div className="flex flex-col gap-1">
            <span>No ready semantic indexes</span>
            <span className="text-content2 text-xs">Create an embedding index from the Indexes page.</span>
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
        onChange={(event) => $aiSearchPrompt.set(event.target.value)}
        placeholder={
          platformSettings?.llmEnabled === false ? 'AI search is not configured' : 'Ask in plain English'
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
            variant="accent"
          >
            {generateSearchQuery.isPending ?
              <LoaderCircle className="animate-spin" />
            : <Play />}
            Run
          </Button>
        }
      >
        <div className="text-2xs text-content flex items-center gap-1 uppercase">Run AI search</div>
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
        View JSON
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
      <div className="text-2xs text-content flex items-center gap-1 uppercase">Search JSON</div>
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
