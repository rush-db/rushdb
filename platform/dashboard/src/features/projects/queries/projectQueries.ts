import { queryOptions } from '@tanstack/react-query'
import type { OrderDirection } from '@rushdb/javascript-sdk'

import type { Filter } from '~/features/search/types'
import type { FiltersCombineMode, Sort } from '~/types'

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
}

export const filteredRecordsQueryOptions = (params: RecordQueryParams) =>
  queryOptions({
    queryKey: queryKeys.projects.records(params.projectId!, {
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
      return api.records.find(
        {
          where:
            params.combineMode === 'or' ?
              { $or: convertToSearchQuery(properties) }
            : convertToSearchQuery(properties),
          orderBy: order,
          skip: params.skip,
          limit: params.limit,
          labels: params.labels
        },
        { signal } as RequestInit
      )
    },
    enabled: !!params.projectId,
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

export type ProjectFieldsQueryParams = {
  projectId: string | undefined
  labels: string[]
  combineMode: FiltersCombineMode
  filters: Filter[]
}

export const projectFieldsQueryOptions = (params: ProjectFieldsQueryParams) =>
  queryOptions({
    queryKey: queryKeys.projects.fields(params.projectId!, {
      labels: params.labels,
      combineMode: params.combineMode,
      filters: params.filters
    }),
    queryFn: async ({ signal }) => {
      let properties
      if (params.combineMode === 'and') {
        properties = params.filters.map(filterToSearchOperation)
      }
      const result = await api.properties.find({
        searchQuery: {
          labels: params.labels,
          where: convertToSearchQuery(properties)
        },
        init: { signal } as RequestInit
      })
      return result.data
    },
    enabled: !!params.projectId
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
