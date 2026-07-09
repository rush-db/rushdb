import { queryOptions } from '@tanstack/react-query'
import type { OrderDirection, SearchQuery } from '@rushdb/javascript-sdk'

import type { Filter } from '~/features/search/types'
import type { FiltersCombineMode, Sort } from '~/types'
import type { EmbeddingIndex } from '~/features/indexes/types'

import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'
import { convertToSearchQuery, filterToSearchOperation } from '~/features/projects/utils'

export const currentProjectQueryOptions = (projectId: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.projects.detail(projectId!),
    queryFn: () => api.projects.project({ projectId: projectId! }, {} as RequestInit),
    enabled: !!projectId
  })

export const projectLabelsQueryOptions = (projectId: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.projects.labels(projectId!),
    queryFn: async () => {
      const result = await api.labels.find({})
      return result.data
    },
    enabled: !!projectId
  })

export const projectTokensQueryOptions = (projectId: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.projects.tokens(projectId!),
    queryFn: () => api.tokens.list({ projectId: projectId! }, {} as RequestInit),
    enabled: !!projectId
  })

export const projectIndexesQueryOptions = (projectId: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.projects.indexes(projectId!),
    queryFn: () => api.indexes.list({ projectId: projectId! }),
    refetchInterval: (query) => {
      const indexes = query.state.data
      if (!Array.isArray(indexes)) return false

      return indexes.some((index) => index.status === 'pending' || index.status === 'indexing') ? 5000 : false
    },
    enabled: !!projectId
  })

export const indexStatsQueryOptions = (indexId: string | undefined, projectId: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.projects.indexStats(projectId!, indexId!),
    queryFn: () => api.indexes.stats({ id: indexId!, projectId: projectId! }),
    enabled: !!indexId && !!projectId,
    refetchInterval: 5000,
    staleTime: 0
  })

function buildOrderObject(orderBy: Sort | undefined): Sort {
  if (!orderBy) return {}
  if (typeof orderBy === 'string') {
    return orderBy as OrderDirection
  }

  const sortMap: Partial<Record<string, OrderDirection>> = {}

  for (const [key, direction] of Object.entries(orderBy)) {
    if (key && direction) {
      sortMap[key] = direction as OrderDirection
    }
  }

  return sortMap
}

export type RecordQueryParams = {
  projectId: string | undefined
  filters: Filter[]
  orderBy: Sort | undefined
  skip: number
  limit: number
  labels: string[]
  combineMode: FiltersCombineMode
  searchMode?: 'manual' | 'ai' | 'semantic'
  searchQuery?: SearchQuery
  semanticSearch?: {
    index?: EmbeddingIndex
    query: string
  }
}

const GRAPH_RELATION_PAGE_SIZE = 1000
const GRAPH_RELATION_EDGE_BUDGET = 10_000

export function buildManualRecordsSearchQuery(params: RecordQueryParams): SearchQuery {
  const properties = params.filters.map(filterToSearchOperation)
  const order = buildOrderObject(params.orderBy)
  return {
    where:
      params.combineMode === 'or' ?
        { $or: convertToSearchQuery(properties) }
      : convertToSearchQuery(properties),
    orderBy: order,
    skip: params.skip,
    limit: params.limit,
    labels: params.labels
  }
}

export function buildRecordsSearchQuery(params: RecordQueryParams): SearchQuery {
  if (params.searchMode === 'ai' && params.searchQuery) {
    if (params.searchQuery.select) {
      return params.searchQuery
    }

    return {
      ...params.searchQuery,
      skip: params.skip,
      limit: params.limit
    }
  }

  return buildManualRecordsSearchQuery(params)
}

function isSemanticSearchActive(params: RecordQueryParams) {
  return Boolean(
    params.searchMode === 'semantic' && params.semanticSearch?.index && params.semanticSearch.query.trim()
  )
}

function getEffectiveRecordsSearchMode(params: RecordQueryParams) {
  if (params.searchMode === 'ai' && !params.searchQuery) {
    return 'manual'
  }

  if (params.searchMode === 'semantic' && !isSemanticSearchActive(params)) {
    return 'manual'
  }

  return params.searchMode
}

export const filteredRecordsQueryOptions = (params: RecordQueryParams) =>
  queryOptions({
    queryKey: queryKeys.projects.records(params.projectId!, {
      filters: params.filters,
      orderBy: params.orderBy,
      skip: params.skip,
      limit: params.limit,
      labels: params.labels,
      combineMode: params.combineMode,
      searchMode: getEffectiveRecordsSearchMode(params),
      searchQuery: params.searchQuery,
      semanticSearch: isSemanticSearchActive(params) ? params.semanticSearch : undefined
    }),
    queryFn: async ({ signal }) => {
      if (isSemanticSearchActive(params) && params.semanticSearch?.index) {
        const index = params.semanticSearch.index
        return api.records.vectorSearch({
          labels: [index.label],
          propertyName: index.propertyName,
          query: params.semanticSearch.query.trim(),
          sourceType: index.sourceType,
          skip: params.skip,
          limit: params.limit
        })
      }

      return api.records.find(buildRecordsSearchQuery(params), { signal } as RequestInit)
    },
    enabled: !!params.projectId,
    retry: false,
    staleTime: 0
  })

export const filteredRecordRelationsQueryOptions = (params: RecordQueryParams) =>
  queryOptions({
    queryKey: queryKeys.projects.recordRelations(params.projectId!, {
      filters: params.filters,
      orderBy: params.orderBy,
      skip: params.skip,
      limit: params.limit,
      labels: params.labels,
      combineMode: params.combineMode
    }),
    queryFn: async ({ signal }) => {
      const properties = params.filters.map(filterToSearchOperation)
      const order = buildOrderObject(params.orderBy)
      return api.relationships.find({
        searchQuery: {
          source: {
            labels: params.labels,
            where:
              params.combineMode === 'or' ?
                { $or: convertToSearchQuery(properties) }
              : convertToSearchQuery(properties)
          },
          orderBy: order,
          skip: params.skip,
          limit: params.limit
        },
        init: { signal } as RequestInit
      })
    },
    enabled: !!params.projectId,
    staleTime: 0
  })

export const graphRecordRelationsQueryOptions = ({
  projectId,
  recordIds
}: {
  projectId: string | undefined
  recordIds: string[]
}) =>
  queryOptions({
    queryKey: queryKeys.projects.graphRecordRelations(projectId!, recordIds),
    queryFn: async ({ signal }) => {
      const visibleRecordIds = new Set(recordIds)
      const relationshipsByKey = new Map<
        string,
        Awaited<ReturnType<typeof api.relationships.find>>['data'][number]
      >()
      let skip = 0
      let firstPage: Awaited<ReturnType<typeof api.relationships.find>> | undefined
      let edgeLimitReached = false

      while (relationshipsByKey.size < GRAPH_RELATION_EDGE_BUDGET) {
        const limit = Math.min(GRAPH_RELATION_PAGE_SIZE, GRAPH_RELATION_EDGE_BUDGET - relationshipsByKey.size)
        const page = await api.relationships.find({
          searchQuery: {
            source: {
              where: {
                $id: {
                  $in: recordIds
                }
              }
            },
            target: {
              where: {
                $id: {
                  $in: recordIds
                }
              }
            },
            skip,
            limit
          },
          init: { signal } as RequestInit
        })

        firstPage ??= page

        const pageData = page.data ?? []
        for (const relationship of pageData) {
          if (!visibleRecordIds.has(relationship.sourceId) || !visibleRecordIds.has(relationship.targetId)) {
            continue
          }

          relationshipsByKey.set(
            `${relationship.sourceId}:${relationship.type}:${relationship.targetId}`,
            relationship
          )
        }

        if (pageData.length < GRAPH_RELATION_PAGE_SIZE) {
          break
        }

        if (relationshipsByKey.size >= GRAPH_RELATION_EDGE_BUDGET) {
          edgeLimitReached = pageData.length === limit
          break
        }

        skip += GRAPH_RELATION_PAGE_SIZE
      }

      const deduped = Array.from(relationshipsByKey.values())

      return {
        ...(firstPage ?? { success: true }),
        data: deduped,
        total: deduped.length,
        edgeLimitReached,
        edgeLimit: GRAPH_RELATION_EDGE_BUDGET
      }
    },
    enabled: !!projectId && recordIds.length > 0,
    staleTime: 0
  })

export type ProjectFieldsQueryParams = {
  projectId: string | undefined
  labels: string[]
  combineMode: FiltersCombineMode
  filters: Filter[]
  /** Full search query driving the records view (AI/smart search) — mirrored so properties match the visible result set. */
  searchQuery?: SearchQuery
}

function buildFieldsSearchQuery(params: ProjectFieldsQueryParams): SearchQuery {
  if (params.searchQuery) {
    return {
      labels: params.searchQuery.labels ?? [],
      where: params.searchQuery.where ?? {}
    }
  }

  const properties = params.filters.map(filterToSearchOperation)
  return {
    labels: params.labels,
    where:
      params.combineMode === 'or' ?
        { $or: convertToSearchQuery(properties) }
      : convertToSearchQuery(properties)
  }
}

export const projectFieldsQueryOptions = (params: ProjectFieldsQueryParams) =>
  queryOptions({
    queryKey: queryKeys.projects.fields(params.projectId!, {
      labels: params.labels,
      combineMode: params.combineMode,
      filters: params.filters,
      searchQuery: params.searchQuery
    }),
    queryFn: async ({ signal }) => {
      const result = await api.properties.find({
        searchQuery: buildFieldsSearchQuery(params),
        init: { signal } as RequestInit
      })
      return result.data
    },
    enabled: !!params.projectId,
    refetchOnWindowFocus: false,
    staleTime: 5 * 60 * 1000
  })

export const recordDetailQueryOptions = (recordId: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.projects.record(recordId!),
    queryFn: async () => {
      const result = await api.records.findById({ id: recordId!, init: {} as RequestInit })
      return result.data
    },
    enabled: !!recordId
  })

export const recordRelatedQueryOptions = (recordId: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.projects.recordRelated(recordId!),
    queryFn: async () => {
      const [outgoing, incoming] = await Promise.all([
        api.relationships.find({
          searchQuery: { source: { where: { $id: recordId! } } },
          init: {} as RequestInit
        }),
        api.relationships.find({
          searchQuery: { target: { where: { $id: recordId! } } },
          init: {} as RequestInit
        })
      ])
      const data = [...(outgoing.data ?? []), ...(incoming.data ?? [])]
      const deduped = Array.from(
        new Map(
          data.map((relationship) => [
            `${relationship.sourceId}:${relationship.type}:${relationship.targetId}`,
            relationship
          ])
        ).values()
      )

      return {
        ...outgoing,
        data: deduped,
        total: deduped.length
      }
    },
    enabled: !!recordId
  })

export const recordFieldsQueryOptions = (recordId: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.projects.recordFields(recordId!),
    queryFn: async () => {
      const result = await api.properties.find({
        searchQuery: { where: { $id: recordId! } },
        init: {} as RequestInit
      })
      return result.data
    },
    enabled: !!recordId
  })

export type ProjectSuggestedFieldsQueryParams = {
  projectId: string | undefined
  labels: string[]
  filters: Filter[]
}

export const projectSuggestedFieldsQueryOptions = (params: ProjectSuggestedFieldsQueryParams) =>
  queryOptions({
    queryKey: queryKeys.projects.suggestedFields(params.projectId!, {
      labels: params.labels,
      filters: params.filters
    }),
    queryFn: async ({ signal }) => {
      const result = await api.properties.find({
        searchQuery: { labels: params.labels },
        init: { signal } as RequestInit
      })
      return result.data
    },
    enabled: !!params.projectId
  })
