import { persistentAtom } from '@nanostores/persistent'
import { nanoid } from 'nanoid'
import { action, atom, computed } from 'nanostores'

import type { AnySearchOperation, Filter } from '~/features/search/types'
import type { SearchParams } from '~/lib/router'
import { type FiltersCombineMode, type Sort, SortDirection } from '~/types'

import { DEFAULT_LIMIT } from '~/config'
import { isViableSearchOperation } from '~/features/search/types'
import { api } from '~/lib/api'
import { createAsyncStore, createMutator } from '~/lib/fetcher'
import { $searchParams, changeSearchParam, removeSearchParam } from '~/lib/router'
import { $router, isProjectPage } from '~/lib/router'
import { addOrRemove, clamp } from '~/lib/utils'

import { RawApiEntityType, RecordViewType } from '../types'

import {
  convertToSearchQuery,
  decodeQuery,
  encodeQuery,
  filterToSearchOperation,
  isProjectEmpty
} from '../utils'
import { $currentProjectId } from './id'

export const $recordView = atom<RecordViewType>('table')

export const $recordRawApiEntity = persistentAtom<RawApiEntityType>('records:raw-api:entity', 'records')

export const $recordsOrderBy = atom<Sort>()

export const $currentProjectRecordsSkip = atom<number>(0)

export const $currentProjectRecordsLimit = atom<number>(DEFAULT_LIMIT)

export const $combineFilters = persistentAtom<FiltersCombineMode>('records:combine-mode', 'and')

export const $currentProject = createAsyncStore({
  key: '$currentProject',
  async fetcher(init) {
    const projectId = $currentProjectId.get()

    if (!projectId) {
      return
    }

    return await api.projects.project({ projectId }, init)
  },
  deps: [$currentProjectId]
})

export const $currentProjectFilters = atom<Filter[]>([])

export const $currentProjectLabels = createAsyncStore({
  key: '$currentProject',
  async fetcher(init) {
    return await api.labels.find({ init })
  },
  mustHaveDeps: [$currentProjectId]
})

export const $activeLabels = atom<string[]>([])

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

export const $filteredRecords = createAsyncStore({
  key: '$projectFilteredRecords',
  async fetcher(init) {
    const filters = $currentProjectFilters.get()
    const orderBy = $recordsOrderBy.get()
    const skip = $currentProjectRecordsSkip.get()
    const limit = $currentProjectRecordsLimit.get()
    const labels = $activeLabels.get()
    const combineMode = $combineFilters.get()
    const properties = filters.map(filterToSearchOperation)

    const order = Object.entries(orderBy ?? {}).reduce<Sort>((acc, [key, direction]) => {
      if (key === '__id') {
        return direction as SortDirection
      }

      if (key && direction) {
        // @ts-ignore
        acc[key] = direction as SortDirection
      }
      return acc
    }, {})

    const { data, total } = await api.records.find(
      {
        where:
          combineMode === 'or' ? { $or: convertToSearchQuery(properties) } : convertToSearchQuery(properties),
        orderBy: order,
        skip,
        limit,
        labels
      },
      init
    )
    return { data, total }
  },
  mustHaveDeps: [$currentProjectId],
  deps: [
    $currentProjectFilters,
    $recordsOrderBy,
    $currentProjectRecordsSkip,
    $currentProjectRecordsLimit,
    $activeLabels,
    $combineFilters
  ]
})

export const $filteredRecordsRelations = createAsyncStore({
  key: '$currentRecordChildren',
  async fetcher(init) {
    const filters = $currentProjectFilters.get()
    const orderBy = $recordsOrderBy.get()
    const skip = $currentProjectRecordsSkip.get()
    const limit = $currentProjectRecordsLimit.get()
    const labels = $activeLabels.get()
    const combineMode = $combineFilters.get()
    const properties = filters.map(filterToSearchOperation)

    const order = Object.entries(orderBy ?? {}).reduce<Sort>((acc, [key, direction]) => {
      if (key === '__id') {
        return direction as SortDirection
      }

      if (key && direction) {
        // @ts-ignore
        acc[key] = direction as SortDirection
      }
      return acc
    }, {})

    const { data, total } = await api.relationships.find({
      searchQuery: {
        where:
          combineMode === 'or' ? { $or: convertToSearchQuery(properties) } : convertToSearchQuery(properties),
        orderBy: order,
        skip,
        limit,
        labels
      },
      init
    })
    return { data, total }
  },
  mustHaveDeps: [$currentProjectId],
  deps: [
    $currentProjectFilters,
    $recordsOrderBy,
    $currentProjectRecordsSkip,
    $currentProjectRecordsLimit,
    $activeLabels,
    $combineFilters
  ]
})

export const $currentProjectFields = createAsyncStore({
  key: '$currentProjectFields',
  async fetcher(init) {
    const projectId = $currentProjectId.get()

    if (!projectId) {
      return
    }

    const labels = $activeLabels.get()
    const combineMode = $combineFilters.get()
    let properties

    if (combineMode === 'and') {
      // Fetch Properties that don't exist with $and grouping
      properties = $currentProjectFilters.get().map(filterToSearchOperation)
    }

    return await api.properties.find({
      searchQuery: {
        labels,
        where: convertToSearchQuery(properties)
      },
      init
    })
  },
  deps: [$combineFilters, $currentProjectId, $activeLabels, $currentProjectFilters]
})

export const $currentProjectSuggestedFields = createAsyncStore({
  key: '$currentProjectSuggestedFields',
  async fetcher(init) {
    const projectId = $currentProjectId.get()

    if (!projectId) {
      return
    }

    const labels = $activeLabels.get()

    let properties

    return await api.properties.find({
      searchQuery: {
        labels,
        where: properties
      },
      init
    })
  },
  deps: [$currentProjectId, $activeLabels, $currentProjectFilters]
})

export const incrementRecordsPage = action($currentProjectRecordsSkip, 'incrementPage', (store) => {
  const limit = $currentProjectRecordsLimit.get()
  const total = $filteredRecords.get().total

  return store.set(clamp(0, total ?? Infinity, store.get() + limit))
})
export const decrementRecordsPage = action($currentProjectRecordsSkip, 'incrementPage', (store) => {
  const limit = $currentProjectRecordsLimit.get()
  const total = $filteredRecords.get().total

  store.set(clamp(0, total ?? Infinity, store.get() - limit))
})

export const $currentProjectTokens = createAsyncStore({
  key: '$currentProjectTokens',
  async fetcher(init) {
    const projectId = $currentProjectId.get()

    if (!projectId) {
      return
    }

    return await api.tokens.list({ projectId }, init)
  }
})

export const $currentProjectIsEmpty = computed([$filteredRecords], ({ total, loading }) => {
  return isProjectEmpty({ totalRecords: total, loading })
})

export const $currentProjectFirstToken = computed([$currentProjectTokens], ({ data, loading }) => {
  return {
    token: data?.[0],
    loading
  }
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

    const field = $currentProjectSuggestedFields.get().data?.find((field) => field.name === newFilter.name)

    //  validate

    if (!field) {
      return currentFilter
    }

    if (
      !isViableSearchOperation({
        propertyType: field.type,
        searchOperation: newFilter.operation
      })
    ) {
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

// @TODO: Consider refactoring here
export const $export = createMutator({
  async fetcher({ init }) {
    const projectId = $currentProjectId.get()

    if (!projectId) {
      return
    }

    const labels = $activeLabels.get()
    const combineMode = $combineFilters.get()
    const orderBy = $recordsOrderBy.get()
    let properties

    if (combineMode === 'and') {
      properties = $currentProjectFilters.get().map(filterToSearchOperation)
    }

    return await api.records.export(
      {
        labels,
        where: convertToSearchQuery(properties),
        orderBy
      },
      init
    )
  },
  onSuccess(response) {
    const anchor = document.createElement('a')
    anchor.setAttribute('href', 'data:attachment/csv;charset=utf-8,' + encodeURI(response.data.fileContent))
    anchor.setAttribute('target', '_blank')
    anchor.setAttribute('download', `collect-export_${response.data.dateTime}.csv`)
    anchor.click()
    anchor.remove()
  }
})

// effects

$router.subscribe((page) => {
  if (isProjectPage(page)) {
    $currentProjectId.set(page.params.id)
  } else {
    $currentProject.setKey('data', undefined)
    $currentProjectId.set(undefined)
  }
})

$currentProjectFilters.subscribe(() => {
  $currentProjectRecordsSkip.set(0)
})

$recordsOrderBy.subscribe(() => {
  $currentProjectRecordsSkip.set(0)
})
