import { useQuery } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

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
  currentProjectQueryOptions,
  projectLabelsQueryOptions,
  projectTokensQueryOptions,
  projectIndexesQueryOptions,
  indexStatsQueryOptions,
  filteredRecordsQueryOptions,
  filteredRecordRelationsQueryOptions,
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

export const useProjectLabelsQuery = () => {
  const projectId = useStore($currentProjectId)
  return useQuery(projectLabelsQueryOptions(projectId))
}

export const useProjectTokensQuery = () => {
  const projectId = useStore($currentProjectId)
  return useQuery(projectTokensQueryOptions(projectId))
}

export const useProjectIndexesQuery = () => {
  const projectId = useStore($currentProjectId)
  return useQuery(projectIndexesQueryOptions(projectId))
}

export const useIndexStatsQuery = (indexId: string) => {
  const projectId = useStore($currentProjectId)
  return useQuery(indexStatsQueryOptions(indexId, projectId))
}

function useRecordQueryParams() {
  const projectId = useStore($currentProjectId)
  const filters = useStore($currentProjectFilters)
  const orderBy = useStore($recordsOrderBy)
  const skip = useStore($currentProjectRecordsSkip)
  const limit = useStore($currentProjectRecordsLimit)
  const labels = useStore($activeLabels)
  const combineMode = useStore($combineFilters)
  return { projectId, filters, orderBy, skip, limit, labels, combineMode }
}

export const useFilteredRecordsQuery = () => {
  const params = useRecordQueryParams()
  return useQuery(filteredRecordsQueryOptions(params))
}

export const useFilteredRecordRelationsQuery = () => {
  const params = useRecordQueryParams()
  return useQuery(filteredRecordRelationsQueryOptions(params))
}

export const useProjectFieldsQuery = () => {
  const projectId = useStore($currentProjectId)
  const labels = useStore($activeLabels)
  const combineMode = useStore($combineFilters)
  const filters = useStore($currentProjectFilters)
  return useQuery(projectFieldsQueryOptions({ projectId, labels, combineMode, filters }))
}

export const useProjectSuggestedFieldsQuery = () => {
  const projectId = useStore($currentProjectId)
  const labels = useStore($activeLabels)
  const filters = useStore($currentProjectFilters)
  return useQuery(projectSuggestedFieldsQueryOptions({ projectId, labels, filters }))
}

export const useCurrentRecordQuery = () => {
  const sheetId = useStore($sheetRecordId)
  const routeId = useStore($currentRecordId)
  const id = sheetId ?? routeId
  return useQuery(recordDetailQueryOptions(id))
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
