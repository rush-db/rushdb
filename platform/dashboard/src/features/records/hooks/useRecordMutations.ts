import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import type { DBRecord, DBRecordCreationOptions, PropertyDraft } from '@rushdb/javascript-sdk'

import { toast } from '~/elements/Toast'
import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'
import { $currentProjectId, $sheetRecordId } from '~/features/projects/stores/id'
import { $currentWorkspaceId } from '~/features/workspaces/stores/current'
import {
  $currentProjectFilters,
  $recordsOrderBy,
  $currentProjectRecordsSkip,
  $currentProjectRecordsLimit,
  $activeLabels,
  $combineFilters
} from '~/features/projects/stores/current-project'
import { $selectedRecords, resetRecordsSelection } from '~/features/records/stores/actionbar'
import {
  $selectedRelatedRecords,
  resetRelatedRecordsSelection
} from '~/features/records/stores/related-actionbar'
import { convertToSearchQuery, filterToSearchOperation } from '~/features/projects/utils'

function useCurrentRecordParams() {
  const projectId = useStore($currentProjectId)
  const filters = useStore($currentProjectFilters)
  const orderBy = useStore($recordsOrderBy)
  const skip = useStore($currentProjectRecordsSkip)
  const limit = useStore($currentProjectRecordsLimit)
  const labels = useStore($activeLabels)
  const combineMode = useStore($combineFilters)
  return { projectId, filters, orderBy, skip, limit, labels, combineMode }
}

function invalidateProjectDataAfterRecordStructureChange(
  queryClient: ReturnType<typeof useQueryClient>,
  projectId: string,
  workspaceId?: string
) {
  queryClient.invalidateQueries({ queryKey: queryKeys.projects.labels(projectId) })
  queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'fields'] })
  queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'suggested-fields'] })
  queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'records'] })
  queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'record-relations'] })
  queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'graph-record-relations'] })
  queryClient.invalidateQueries({ queryKey: queryKeys.projects.relationshipPatterns(projectId) })

  invalidateProjectStats(queryClient, projectId, workspaceId)
}

function invalidateProjectStats(
  queryClient: ReturnType<typeof useQueryClient>,
  projectId: string,
  workspaceId?: string
) {
  queryClient.invalidateQueries({ queryKey: queryKeys.projects.detail(projectId) })
  queryClient.invalidateQueries({ queryKey: queryKeys.projects.stats(projectId) })
  queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.all() })

  if (workspaceId) {
    queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.detail(workspaceId) })
    queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.projects(workspaceId) })
  }
}

export const useDeleteRecordMutation = () => {
  const queryClient = useQueryClient()
  const params = useCurrentRecordParams()
  return useMutation({
    mutationFn: ({ id }: { id: DBRecord['__id'] }) => api.records.deleteById({ id }),
    onSuccess(_, { id }) {
      if (!params.projectId) return
      if ($sheetRecordId.get() === id) {
        $sheetRecordId.set(undefined)
      }
      queryClient.removeQueries({ queryKey: queryKeys.projects.record(id) })
      queryClient.removeQueries({ queryKey: queryKeys.projects.recordFields(id) })
      queryClient.removeQueries({ queryKey: queryKeys.projects.recordRelated(id) })
      queryClient.invalidateQueries({ queryKey: queryKeys.projects.labels(params.projectId) })
      queryClient.invalidateQueries({
        queryKey: queryKeys.projects.fields(params.projectId, {
          labels: params.labels,
          combineMode: params.combineMode,
          filters: params.filters
        })
      })
      queryClient.invalidateQueries({
        queryKey: queryKeys.projects.records(params.projectId, {
          filters: params.filters,
          orderBy: params.orderBy,
          skip: params.skip,
          limit: params.limit,
          labels: params.labels,
          combineMode: params.combineMode
        })
      })
    }
  })
}

export const useExportRecordsMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  const labels = useStore($activeLabels)
  const combineMode = useStore($combineFilters)
  const orderBy = useStore($recordsOrderBy)
  const skip = useStore($currentProjectRecordsSkip)
  const limit = useStore($currentProjectRecordsLimit)
  const filters = useStore($currentProjectFilters)

  return useMutation({
    mutationFn: async () => {
      if (!projectId) return
      let properties
      if (combineMode === 'and') {
        properties = filters.map(filterToSearchOperation)
      }
      return api.records.export(
        { labels, where: convertToSearchQuery(properties), orderBy, skip, limit },
        {} as RequestInit
      )
    },
    onSuccess(response) {
      if (!response) return
      const anchor = document.createElement('a')
      anchor.setAttribute('href', 'data:attachment/csv;charset=utf-8,' + encodeURI(response.data.fileContent))
      anchor.setAttribute('target', '_blank')
      anchor.setAttribute('download', `collect-export_${response.data.dateTime}.csv`)
      anchor.click()
      anchor.remove()
    }
  })
}

export const useImportJsonMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  const workspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: (params: {
      label?: string
      data: Parameters<typeof api.records.importJson>[0]['data']
      options?: DBRecordCreationOptions
    }) => api.records.importJson(params),
    onSuccess() {
      if (!projectId) return
      invalidateProjectDataAfterRecordStructureChange(queryClient, projectId, workspaceId)
    }
  })
}

export const useImportCsvMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  const workspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: (params: {
      label: string
      data: string
      options?: DBRecordCreationOptions
      parseConfig?: {
        delimiter?: string
        header?: boolean
        skipEmptyLines?: boolean | 'greedy'
        dynamicTyping?: boolean
        quoteChar?: string
        escapeChar?: string
        newline?: string
      }
    }) => api.records.importCsv(params),
    onSuccess() {
      if (!projectId) return
      invalidateProjectDataAfterRecordStructureChange(queryClient, projectId, workspaceId)
    }
  })
}

export const useBatchDeleteRecordsMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  const selectedRecords = useStore($selectedRecords)

  return useMutation({
    mutationFn: async () => {
      if (!selectedRecords.length) {
        throw new Error('No records selected')
      }
      return api.records.delete({ ids: selectedRecords as Array<string> })
    },
    onSuccess() {
      if (!projectId) return
      queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'records'] })
      queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'labels'] })
      queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'fields'] })
      resetRecordsSelection()
      toast({
        title: 'Records were successfully deleted'
      })
    }
  })
}

export const useSetRecordMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)

  return useMutation({
    mutationFn: ({ id, label, data }: { id: string; label: string; data: PropertyDraft[] }) =>
      api.records.set(id, label, data, {} as RequestInit),
    onSuccess(_, { id }) {
      queryClient.invalidateQueries({ queryKey: queryKeys.projects.record(id) })
      if (!projectId) return
      queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'records'] })
      queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'fields'] })
    }
  })
}

export const useUpdateRecordMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)

  return useMutation({
    mutationFn: ({ id, label, data }: { id: string; label: string; data: PropertyDraft[] }) =>
      api.records.update(id, label, data, {} as RequestInit),
    onSuccess(_, { id }) {
      queryClient.invalidateQueries({ queryKey: queryKeys.projects.record(id) })
      if (!projectId) return
      queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'records'] })
      queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'fields'] })
    }
  })
}

export const useBatchDeleteRelatedRecordsMutation = () => {
  const queryClient = useQueryClient()
  const projectId = useStore($currentProjectId)
  const selectedRelatedRecords = useStore($selectedRelatedRecords)

  return useMutation({
    mutationFn: async () => {
      if (!selectedRelatedRecords.length) {
        throw new Error('No records selected')
      }
      return api.records.delete({ ids: selectedRelatedRecords as Array<string> })
    },
    onSuccess() {
      if (!projectId) return
      queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'records'] })
      queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'record-relations'] })
      queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'labels'] })
      queryClient.invalidateQueries({ queryKey: ['projects', projectId, 'fields'] })
      resetRelatedRecordsSelection()
      toast({
        title: 'Records were successfully deleted'
      })
    }
  })
}
