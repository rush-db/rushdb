import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import type { SearchQuery } from '@rushdb/javascript-sdk'

import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'
import { toast } from '~/elements/Toast'
import { $currentProjectId, $sheetRecordId, $currentRecordId } from '~/features/projects/stores/id'
import {
  $currentProjectFilters,
  $recordsOrderBy,
  $currentProjectRecordsSkip,
  $currentProjectRecordsLimit,
  $activeLabels,
  $combineFilters
} from '~/features/projects/stores/current-project'
import {
  $aiSearchQuery,
  $semanticSearchIndexId,
  $semanticSearchPrompt,
  $searchQueryModalOpen,
  $recordsSearchMode,
  setDraftSearchQuery
} from '~/features/projects/stores/records-search'
import {
  buildManualRecordsSearchQuery,
  buildRecordsSearchQuery,
  currentProjectQueryOptions,
  projectLabelsQueryOptions,
  projectTokensQueryOptions,
  projectIndexesQueryOptions,
  indexStatsQueryOptions,
  filteredRecordsQueryOptions,
  filteredRecordRelationsQueryOptions,
  graphRecordRelationsQueryOptions,
  projectFieldsQueryOptions,
  projectSuggestedFieldsQueryOptions,
  recordDetailQueryOptions,
  recordRelatedQueryOptions,
  recordFieldsQueryOptions
} from '../queries/projectQueries'

export const useCurrentProjectQuery = () => {
  const projectId = useStore($currentProjectId)
  return useQuery(currentProjectQueryOptions(projectId))
}

export const useProjectLabelsQuery = ({ enabled = true }: { enabled?: boolean } = {}) => {
  const projectId = useStore($currentProjectId)
  return useQuery({
    ...projectLabelsQueryOptions(projectId),
    enabled: enabled && !!projectId
  })
}

export const useProjectTokensQuery = () => {
  const projectId = useStore($currentProjectId)
  return useQuery(projectTokensQueryOptions(projectId))
}

export const useProjectIndexesQuery = ({ enabled = true }: { enabled?: boolean } = {}) => {
  const projectId = useStore($currentProjectId)
  return useQuery({
    ...projectIndexesQueryOptions(projectId),
    enabled: enabled && !!projectId
  })
}

export const useIndexStatsQuery = (indexId: string) => {
  const projectId = useStore($currentProjectId)
  return useQuery(indexStatsQueryOptions(indexId, projectId))
}

function useRecordQueryParams() {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  const filters = useStore($currentProjectFilters)
  const orderBy = useStore($recordsOrderBy)
  const skip = useStore($currentProjectRecordsSkip)
  const limit = useStore($currentProjectRecordsLimit)
  const labels = useStore($activeLabels)
  const combineMode = useStore($combineFilters)
  const searchMode = useStore($recordsSearchMode)
  const searchQuery = useStore($aiSearchQuery)
  const semanticSearchIndexId = useStore($semanticSearchIndexId)
  const semanticSearchPrompt = useStore($semanticSearchPrompt)
  const cachedIndexes =
    projectId ?
      queryClient.getQueryData<Awaited<ReturnType<typeof api.indexes.list>>>(
        queryKeys.projects.indexes(projectId)
      )
    : undefined
  const semanticIndex = cachedIndexes?.find((index) => index.id === semanticSearchIndexId)

  return {
    projectId,
    filters,
    orderBy,
    skip,
    limit,
    labels,
    combineMode,
    searchMode,
    searchQuery,
    semanticSearch: {
      index: semanticIndex,
      query: semanticSearchPrompt
    }
  }
}

export const useFilteredRecordsQuery = () => {
  const params = useRecordQueryParams()
  return useQuery(filteredRecordsQueryOptions(params))
}

export const useCurrentManualRecordsSearchQuery = () => {
  const params = useRecordQueryParams()
  return buildManualRecordsSearchQuery(params)
}

// Mirrors the query that produced the current view (manual or AI, with pagination),
// used by the server-side CSV export so the download matches what's on screen.
export const useCurrentRecordsSearchQuery = () => {
  const params = useRecordQueryParams()
  return buildRecordsSearchQuery(params)
}

export const useGenerateSearchQueryMutation = () => {
  const projectId = useStore($currentProjectId)
  const currentQuery = useCurrentManualRecordsSearchQuery()

  return useMutation({
    mutationFn: ({ prompt, query }: { prompt: string; query?: SearchQuery }) =>
      api.ai.generateSearchQuery({
        projectId: projectId!,
        prompt,
        currentQuery: query ?? currentQuery
      }),
    onSuccess(result) {
      $currentProjectRecordsSkip.set(0)
      setDraftSearchQuery(result.searchQuery)
      $searchQueryModalOpen.set(true)
      if (result.warnings?.length) {
        toast({
          title: 'Query adjusted',
          description: result.warnings.join(' ')
        })
      }
    },
    onError(error: unknown) {
      if (error && typeof error === 'object' && 'message' in error) {
        toast({
          title: 'AI search failed',
          description: String(error.message),
          variant: 'danger'
        })
      }
    }
  })
}

export const useFilteredRecordRelationsQuery = () => {
  const params = useRecordQueryParams()
  return useQuery(filteredRecordRelationsQueryOptions(params))
}

export const useGraphRecordRelationsQuery = (recordIds: string[]) => {
  const projectId = useStore($currentProjectId)
  return useQuery(graphRecordRelationsQueryOptions({ projectId, recordIds }))
}

export const useProjectFieldsQuery = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  const labels = useStore($activeLabels)
  const combineMode = useStore($combineFilters)
  const filters = useStore($currentProjectFilters)
  const searchMode = useStore($recordsSearchMode)
  const aiSearchQuery = useStore($aiSearchQuery)
  const semanticSearchIndexId = useStore($semanticSearchIndexId)
  const semanticSearchPrompt = useStore($semanticSearchPrompt)
  const cachedIndexes =
    projectId ?
      queryClient.getQueryData<Awaited<ReturnType<typeof api.indexes.list>>>(
        queryKeys.projects.indexes(projectId)
      )
    : undefined
  const semanticIndex = cachedIndexes?.find((index) => index.id === semanticSearchIndexId)
  const semanticSearchActive = Boolean(
    searchMode === 'semantic' && semanticIndex?.label && semanticSearchPrompt.trim()
  )
  const aiSearchActive = searchMode === 'ai' && !!aiSearchQuery
  const scopedLabels = semanticSearchActive && semanticIndex?.label ? [semanticIndex.label] : labels
  const scopedFilters = semanticSearchActive ? [] : filters

  return useQuery(
    projectFieldsQueryOptions({
      projectId,
      labels: scopedLabels,
      combineMode,
      filters: scopedFilters,
      searchQuery: aiSearchActive ? aiSearchQuery : undefined
    })
  )
}

export const useProjectSuggestedFieldsQuery = () => {
  const projectId = useStore($currentProjectId)
  const labels = useStore($activeLabels)
  const filters = useStore($currentProjectFilters)
  return useQuery(projectSuggestedFieldsQueryOptions({ projectId, labels, filters }))
}

export const useCurrentRecordQuery = () => {
  const queryClient = useQueryClient()
  const sheetId = useStore($sheetRecordId)
  const routeId = useStore($currentRecordId)
  const id = sheetId ?? routeId
  return useQuery({
    ...recordDetailQueryOptions(id),
    // The records table already holds this record's data — show it instantly
    // and let the detail fetch replace it in the background.
    placeholderData: () => {
      if (!id) return undefined
      const listQueries = queryClient.getQueriesData<{ data?: Array<{ __id: string }> }>({
        predicate: (query) => query.queryKey[0] === 'projects' && query.queryKey[2] === 'records'
      })
      for (const [, cached] of listQueries) {
        const match = cached?.data?.find((record) => record.__id === id)
        if (match) {
          return match as Awaited<ReturnType<typeof api.records.findById>>['data']
        }
      }
      return undefined
    }
  })
}

export const useCurrentRecordRelatedQuery = () => {
  const sheetId = useStore($sheetRecordId)
  const routeId = useStore($currentRecordId)
  const id = sheetId ?? routeId
  return useQuery(recordRelatedQueryOptions(id))
}

export const useCurrentRecordFieldsQuery = () => {
  const sheetId = useStore($sheetRecordId)
  const routeId = useStore($currentRecordId)
  const id = sheetId ?? routeId
  return useQuery(recordFieldsQueryOptions(id))
}
