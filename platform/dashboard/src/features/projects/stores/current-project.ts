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
import { queryClient } from '~/lib/queryClient'
import { map, onMount, onNotify } from 'nanostores'
import { isAnyObject } from '~/types'
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
import { LabelsResponse } from '~/features/labels'

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

// Refactored: $currentProjectLabels react-query bridge
async function fetchProjectLabels(): Promise<{ data: LabelsResponse } | undefined> {
  const projectId = $currentProjectId.get()
  if (!projectId) return
  const response = await api.labels.find({} as any)
  return { data: (response as any).data ?? response }
}
export const $currentProjectLabels = map<{
  data: LabelsResponse | undefined
  loading: boolean
  error?: string
}>({
  data: undefined,
  loading: true,
  error: undefined
})
// @ts-ignore
$currentProjectLabels.refetch = async () => {
  const projectId = $currentProjectId.get()
  if (!projectId) {
    $currentProjectLabels.set({ data: undefined, loading: false })
    return
  }
  const queryKey = ['project-labels', projectId]
  $currentProjectLabels.set({ ...$currentProjectLabels.get(), loading: true })
  try {
    const response = await queryClient.fetchQuery({ queryKey, queryFn: fetchProjectLabels })
    if (!response) return
    $currentProjectLabels.set({ data: response.data as LabelsResponse, loading: false })
  } catch (error) {
    if (error instanceof Error) {
      $currentProjectLabels.set({ data: undefined, loading: false, error: error.message })
    }
  }
}
onMount($currentProjectLabels, () => {
  const run = () => {
    const projectId = $currentProjectId.get()
    if (!projectId) {
      $currentProjectLabels.set({ data: undefined, loading: false })
      return
    }
    const queryKey = ['project-labels', projectId]
    $currentProjectLabels.set({ ...$currentProjectLabels.get(), loading: true })
    queryClient
      .fetchQuery({ queryKey, queryFn: fetchProjectLabels })
      .then((response) => {
        if (!response) return
        $currentProjectLabels.set({ data: response.data as LabelsResponse, loading: false })
      })
      .catch((error: unknown) => {
        if (error instanceof Error) {
          $currentProjectLabels.set({ data: undefined, loading: false, error: error.message })
        }
      })
  }
  run()
  return () => {}
})
onNotify($currentProjectId as any, () => queueMicrotask(() => ($currentProjectLabels as any).refetch()))

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

// Refactored: $filteredRecords now backed by react-query while preserving nanostore API (data, loading, total, error, refetch)
type RecordsQueryData = { data: unknown[]; total?: number }

function buildRecordsQueryArgs() {
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

  const where =
    combineMode === 'or' ? { $or: convertToSearchQuery(properties) } : convertToSearchQuery(properties)

  return { where, orderBy: order, skip, limit, labels }
}

async function fetchRecords(): Promise<RecordsQueryData | undefined> {
  const projectId = $currentProjectId.get()
  if (!projectId) return
  const args = buildRecordsQueryArgs()
  const { data, total } = await api.records.find(args as any, { signal: undefined as any })
  return { data, total }
}

// Bridge nanostore
export const $filteredRecords = map<{
  data: any[] | undefined
  loading: boolean
  error?: string
  total?: number
}>({
  data: undefined,
  loading: true,
  error: undefined,
  total: undefined
})

// Maintain refetch method compatibility
// @ts-ignore - augment store
$filteredRecords.refetch = async () => {
  const projectId = $currentProjectId.get()
  if (!projectId) {
    $filteredRecords.set({ data: undefined, loading: false, total: 0 })
    return
  }
  const args = buildRecordsQueryArgs()
  const queryKey = ['records', projectId, JSON.stringify(args)]
  $filteredRecords.set({ ...$filteredRecords.get(), loading: true })
  try {
    const response = await queryClient.fetchQuery({ queryKey, queryFn: fetchRecords })
    if (!response) return
    $filteredRecords.set({ data: response.data as any[], total: response.total, loading: false })
  } catch (error) {
    if (error instanceof Error) {
      $filteredRecords.set({ data: undefined, loading: false, error: error.message, total: 0 })
    }
  }
}

onMount($filteredRecords, () => {
  const run = () => {
    const projectId = $currentProjectId.get()
    if (!projectId) {
      $filteredRecords.set({ data: undefined, loading: false, total: 0 })
      return
    }
    const args = buildRecordsQueryArgs()
    const queryKey = ['records', projectId, JSON.stringify(args)]

    $filteredRecords.set({ ...$filteredRecords.get(), loading: true })

    queryClient
      .fetchQuery({ queryKey, queryFn: fetchRecords })
      .then((response) => {
        if (!response) return
        $filteredRecords.set({ data: response.data as any[], total: response.total, loading: false })
      })
      .catch((error: unknown) => {
        if (error instanceof Error) {
          $filteredRecords.set({ data: undefined, loading: false, error: error.message, total: 0 })
        }
      })
  }

  run()
  return () => {}
})

// Re-run on dependency changes
for (const dep of [
  $currentProjectFilters,
  $recordsOrderBy,
  $currentProjectRecordsSkip,
  $currentProjectRecordsLimit,
  $activeLabels,
  $combineFilters,
  $currentProjectId
]) {
  onNotify(dep as any, () => {
    queueMicrotask(() => ($filteredRecords as any).refetch())
  })
}

// Refactored: $filteredRecordsRelations using react-query bridge
function buildRelationsQueryArgs() {
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

  const where =
    combineMode === 'or' ? { $or: convertToSearchQuery(properties) } : convertToSearchQuery(properties)

  return { where, orderBy: order, skip, limit, labels }
}

async function fetchRelations(): Promise<{ data: unknown[]; total?: number } | undefined> {
  const projectId = $currentProjectId.get()
  if (!projectId) return
  const args = buildRelationsQueryArgs()
  const { data, total } = await api.relationships.find({ searchQuery: args } as any)
  return { data, total }
}

export const $filteredRecordsRelations = map<{
  data: any[] | undefined
  loading: boolean
  error?: string
  total?: number
}>({
  data: undefined,
  loading: true,
  error: undefined,
  total: undefined
})
// @ts-ignore
$filteredRecordsRelations.refetch = async () => {
  const projectId = $currentProjectId.get()
  if (!projectId) {
    $filteredRecordsRelations.set({ data: undefined, loading: false, total: 0 })
    return
  }
  const args = buildRelationsQueryArgs()
  const queryKey = ['record-relations', projectId, JSON.stringify(args)]
  $filteredRecordsRelations.set({ ...$filteredRecordsRelations.get(), loading: true })
  try {
    const response = await queryClient.fetchQuery({ queryKey, queryFn: fetchRelations })
    if (!response) return
    $filteredRecordsRelations.set({ data: response.data as any[], total: response.total, loading: false })
  } catch (error) {
    if (error instanceof Error) {
      $filteredRecordsRelations.set({ data: undefined, loading: false, error: error.message, total: 0 })
    }
  }
}
onMount($filteredRecordsRelations, () => {
  const run = () => {
    const projectId = $currentProjectId.get()
    if (!projectId) {
      $filteredRecordsRelations.set({ data: undefined, loading: false, total: 0 })
      return
    }
    const args = buildRelationsQueryArgs()
    const queryKey = ['record-relations', projectId, JSON.stringify(args)]
    $filteredRecordsRelations.set({ ...$filteredRecordsRelations.get(), loading: true })
    queryClient
      .fetchQuery({ queryKey, queryFn: fetchRelations })
      .then((response) => {
        if (!response) return
        $filteredRecordsRelations.set({ data: response.data as any[], total: response.total, loading: false })
      })
      .catch((error: unknown) => {
        if (error instanceof Error) {
          $filteredRecordsRelations.set({ data: undefined, loading: false, error: error.message, total: 0 })
        }
      })
  }
  run()
  return () => {}
})
for (const dep of [
  $currentProjectFilters,
  $recordsOrderBy,
  $currentProjectRecordsSkip,
  $currentProjectRecordsLimit,
  $activeLabels,
  $combineFilters,
  $currentProjectId
]) {
  onNotify(dep as any, () => queueMicrotask(() => ($filteredRecordsRelations as any).refetch()))
}

// Refactored: $currentProjectFields react-query bridge
function buildFieldsQueryArgs() {
  const labels = $activeLabels.get()
  const combineMode = $combineFilters.get()
  let properties
  if (combineMode === 'and') {
    properties = $currentProjectFilters.get().map(filterToSearchOperation)
  }
  return { labels, where: convertToSearchQuery(properties) }
}
async function fetchFields(): Promise<{ data: any[] } | undefined> {
  const projectId = $currentProjectId.get()
  if (!projectId) return
  const args = buildFieldsQueryArgs()
  const response = await api.properties.find({ searchQuery: args } as any)
  return { data: (response as any).data ?? response }
}
export const $currentProjectFields = map<{
  data: any[] | undefined
  loading: boolean
  error?: string
}>({
  data: undefined,
  loading: true,
  error: undefined
})
// @ts-ignore
$currentProjectFields.refetch = async () => {
  const projectId = $currentProjectId.get()
  if (!projectId) {
    $currentProjectFields.set({ data: undefined, loading: false })
    return
  }
  const args = buildFieldsQueryArgs()
  const queryKey = ['project-fields', projectId, JSON.stringify(args)]
  $currentProjectFields.set({ ...$currentProjectFields.get(), loading: true })
  try {
    const response = await queryClient.fetchQuery({ queryKey, queryFn: fetchFields })
    if (!response) return
    $currentProjectFields.set({ data: response.data as any[], loading: false })
  } catch (error) {
    if (error instanceof Error) {
      $currentProjectFields.set({ data: undefined, loading: false, error: error.message })
    }
  }
}
onMount($currentProjectFields, () => {
  const run = () => {
    const projectId = $currentProjectId.get()
    if (!projectId) {
      $currentProjectFields.set({ data: undefined, loading: false })
      return
    }
    const args = buildFieldsQueryArgs()
    const queryKey = ['project-fields', projectId, JSON.stringify(args)]
    $currentProjectFields.set({ ...$currentProjectFields.get(), loading: true })
    queryClient
      .fetchQuery({ queryKey, queryFn: fetchFields })
      .then((response) => {
        if (!response) return
        $currentProjectFields.set({ data: response.data as any[], loading: false })
      })
      .catch((error: unknown) => {
        if (error instanceof Error) {
          $currentProjectFields.set({ data: undefined, loading: false, error: error.message })
        }
      })
  }
  run()
  return () => {}
})
for (const dep of [$combineFilters, $currentProjectId, $activeLabels, $currentProjectFilters]) {
  onNotify(dep as any, () => queueMicrotask(() => ($currentProjectFields as any).refetch()))
}

// Refactored: $currentProjectSuggestedFields react-query bridge
function buildSuggestedFieldsQueryArgs() {
  const labels = $activeLabels.get()
  return { labels }
}
async function fetchSuggestedFields(): Promise<{ data: any[] } | undefined> {
  const projectId = $currentProjectId.get()
  if (!projectId) return
  const args = buildSuggestedFieldsQueryArgs()
  const response = await api.properties.find({ searchQuery: args } as any)
  return { data: (response as any).data ?? response }
}
export const $currentProjectSuggestedFields = map<{
  data: any[] | undefined
  loading: boolean
  error?: string
}>({
  data: undefined,
  loading: true,
  error: undefined
})
// @ts-ignore
$currentProjectSuggestedFields.refetch = async () => {
  const projectId = $currentProjectId.get()
  if (!projectId) {
    $currentProjectSuggestedFields.set({ data: undefined, loading: false })
    return
  }
  const args = buildSuggestedFieldsQueryArgs()
  const queryKey = ['project-suggested-fields', projectId, JSON.stringify(args)]
  $currentProjectSuggestedFields.set({ ...$currentProjectSuggestedFields.get(), loading: true })
  try {
    const response = await queryClient.fetchQuery({ queryKey, queryFn: fetchSuggestedFields })
    if (!response) return
    $currentProjectSuggestedFields.set({ data: response.data as any[], loading: false })
  } catch (error) {
    if (error instanceof Error) {
      $currentProjectSuggestedFields.set({ data: undefined, loading: false, error: error.message })
    }
  }
}
onMount($currentProjectSuggestedFields, () => {
  const run = () => {
    const projectId = $currentProjectId.get()
    if (!projectId) {
      $currentProjectSuggestedFields.set({ data: undefined, loading: false })
      return
    }
    const args = buildSuggestedFieldsQueryArgs()
    const queryKey = ['project-suggested-fields', projectId, JSON.stringify(args)]
    $currentProjectSuggestedFields.set({ ...$currentProjectSuggestedFields.get(), loading: true })
    queryClient
      .fetchQuery({ queryKey, queryFn: fetchSuggestedFields })
      .then((response) => {
        if (!response) return
        $currentProjectSuggestedFields.set({ data: response.data as any[], loading: false })
      })
      .catch((error: unknown) => {
        if (error instanceof Error) {
          $currentProjectSuggestedFields.set({ data: undefined, loading: false, error: error.message })
        }
      })
  }
  run()
  return () => {}
})
for (const dep of [$currentProjectId, $activeLabels, $currentProjectFilters]) {
  onNotify(dep as any, () => queueMicrotask(() => ($currentProjectSuggestedFields as any).refetch()))
}

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
  // Immediately clear filters store so dependent queries refetch without waiting for router subscription
  $currentProjectFilters.set([])
}

export const removeFilter = (filter: Filter) => {
  const newFilters = $currentProjectFilters.get().filter(({ filterId }) => filterId !== filter.filterId)

  if (Object.keys(newFilters).length === 0) {
    return resetFilters()
  }

  changeSearchParam('query', encodeQuery(newFilters))
  // Proactively update store to trigger reactive refetch (router update may be async or skipped if value unchanged)
  $currentProjectFilters.set(newFilters)
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
