import { persistentAtom } from '@nanostores/persistent'
import { nanoid } from 'nanoid'
import { action, atom } from 'nanostores'

import type { AnySearchOperation, Filter } from '~/features/search/types'
import { type FiltersCombineMode, type Sort } from '~/types'

import { DEFAULT_LIMIT } from '~/config'
import { $searchParams, changeSearchParam, removeSearchParam } from '~/lib/router'
import { $router, isProjectPage } from '~/lib/router'
import { addOrRemove, clamp } from '~/lib/utils'

import { RawApiEntityType, RecordViewType } from '../types'

import { convertToSearchQuery, decodeQuery, encodeQuery, filterToSearchOperation } from '../utils'
import { $currentProjectId } from './id'

export const $recordView = atom<RecordViewType>('table')

export const $recordRawApiEntity = persistentAtom<RawApiEntityType>('records:raw-api:entity', 'records')

export const $recordsOrderBy = atom<Sort>()

export const $currentProjectRecordsSkip = atom<number>(0)

export const $currentProjectRecordsLimit = atom<number>(DEFAULT_LIMIT)

export const $combineFilters = persistentAtom<FiltersCombineMode>('records:combine-mode', 'and')

export const $currentProjectFilters = atom<Filter[]>([])

export const $activeLabels = atom<string[]>([])

// Total records count - set by ProjectRecords component when data loads
export const $filteredRecordsTotal = atom<number | undefined>(undefined)

$currentProjectId.subscribe(() => {
  $currentProjectFilters.set([])
  $activeLabels.set([])
  $currentProjectRecordsSkip.set(0)
  $currentProjectRecordsLimit.set(100)
  $recordsOrderBy.set(undefined)
  $combineFilters.set('and')
})

$searchParams.subscribe((value) => {
  let filters: Filter[]
  if (!('query' in value) || value.query.length < 1) {
    filters = [] satisfies Filter[]
  } else {
    filters = decodeQuery?.(value.query) as Filter[]
  }
  // Workaround to apply filters after project page is loaded and query params isn't empty.
  setTimeout(() => $currentProjectFilters.set(filters), 10)
})

export const incrementRecordsPage = action($currentProjectRecordsSkip, 'incrementPage', (store) => {
  const limit = $currentProjectRecordsLimit.get()
  const total = $filteredRecordsTotal.get()

  return store.set(clamp(0, total ?? Infinity, store.get() + limit))
})
export const decrementRecordsPage = action($currentProjectRecordsSkip, 'incrementPage', (store) => {
  const limit = $currentProjectRecordsLimit.get()
  const total = $filteredRecordsTotal.get()

  store.set(clamp(0, total ?? Infinity, store.get() - limit))
})

export const setRecordsSort = action($recordsOrderBy, 'setRecordsSort', (store, fieldName: string) => {
  const sort = store.get()

  const sortDirection = typeof sort === 'string' ? sort : (sort ?? {})[fieldName]

  const nextSortDirection =
    sortDirection === undefined ? 'asc'
    : sortDirection === 'asc' ? 'desc'
    : undefined

  let newSort: Sort

  if (typeof sort === 'string') {
    newSort = nextSortDirection
  } else {
    newSort = {
      [fieldName]: nextSortDirection
    }
  }

  $recordsOrderBy.set(newSort)
})

export const resetFilters = () => {
  removeSearchParam('query')
}

export const removeFilter = (filter: Filter) => {
  const newFilters = $currentProjectFilters.get().filter(({ filterId }) => filterId !== filter.filterId)

  if (Object.keys(newFilters).length === 0) {
    return resetFilters()
  }

  changeSearchParam('query', encodeQuery(newFilters))
}

export const addFilter = (operation: AnySearchOperation) => {
  const filter = {
    ...operation,
    filterId: nanoid()
  } satisfies Filter

  const newFilters = [...$currentProjectFilters.get()] satisfies Filter[]

  newFilters.push(filter)

  changeSearchParam('query', encodeQuery(newFilters))
}

export const editFilter = (filter: Filter) => {
  const newFilters = $currentProjectFilters.get().map((currentFilter) => {
    if (currentFilter.filterId !== filter.filterId) {
      return currentFilter
    }

    const newFilter = { ...currentFilter, ...filter } satisfies Filter

    if (!newFilter.operation) {
      return currentFilter
    }

    return newFilter
  }) satisfies Filter[]

  changeSearchParam('query', encodeQuery(newFilters))
}

export const toggleLabel = action($activeLabels, 'toggleLabel', (store, labelValue: string) => {
  const labels = store.get()

  store.set(addOrRemove(labels, labelValue))
})

// effects

$router.subscribe((page) => {
  if (isProjectPage(page)) {
    $currentProjectId.set(page.params.id)
  } else {
    $currentProjectId.set(undefined)
  }
})

$currentProjectFilters.subscribe(() => {
  $currentProjectRecordsSkip.set(0)
})

$recordsOrderBy.subscribe(() => {
  $currentProjectRecordsSkip.set(0)
})
