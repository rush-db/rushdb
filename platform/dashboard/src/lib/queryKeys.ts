import type { Filter } from '~/features/search/types'
import type { FiltersCombineMode, Sort } from '~/types'

export const queryKeys = {
  settings: () => ['settings'] as const,

  user: () => ['user'] as const,

  workspaces: {
    all: () => ['workspaces'] as const,
    detail: (id: string) => ['workspaces', id] as const,
    users: (id: string) => ['workspaces', id, 'users'] as const,
    accessList: (id: string) => ['workspaces', id, 'access-list'] as const,
    pendingInvites: (id: string) => ['workspaces', id, 'pending-invites'] as const,
    projects: (id: string) => ['workspaces', id, 'projects'] as const
  },

  projects: {
    detail: (projectId: string) => ['projects', projectId] as const,
    tokens: (projectId: string) => ['projects', projectId, 'tokens'] as const,
    labels: (projectId: string) => ['projects', projectId, 'labels'] as const,
    indexStats: (projectId: string, indexId: string) =>
      ['projects', projectId, 'indexes', indexId, 'stats'] as const,
    semanticSearchTest: (projectId: string, indexId: string) =>
      ['projects', projectId, 'indexes', indexId, 'semantic-search-test'] as const,
    fields: (
      projectId: string,
      params: { labels: string[]; combineMode: FiltersCombineMode; filters: Filter[] }
    ) => ['projects', projectId, 'fields', params] as const,
    suggestedFields: (projectId: string, params: { labels: string[]; filters: Filter[] }) =>
      ['projects', projectId, 'suggested-fields', params] as const,
    indexes: (projectId: string) => ['projects', projectId, 'indexes'] as const,
    stats: (projectId: string) => ['projects', projectId, 'stats'] as const,
    records: (
      projectId: string,
      params: {
        filters: Filter[]
        orderBy: Sort | undefined
        skip: number
        limit: number
        labels: string[]
        combineMode: FiltersCombineMode
      }
    ) => ['projects', projectId, 'records', params] as const,
    recordRelations: (
      projectId: string,
      params: {
        filters: Filter[]
        orderBy: Sort | undefined
        skip: number
        limit: number
        labels: string[]
        combineMode: FiltersCombineMode
      }
    ) => ['projects', projectId, 'record-relations', params] as const,
    record: (recordId: string) => ['records', recordId] as const,
    recordFields: (recordId: string) => ['records', recordId, 'fields'] as const,
    recordRelated: (recordId: string) => ['records', recordId, 'related'] as const
  },

  billing: {
    data: () => ['billing', 'data'] as const,
    usage: (workspaceId: string) => ['billing', 'usage', workspaceId] as const,
    kuHistory: (
      workspaceId: string,
      params: {
        limit?: number
        before?: string
        since?: string
        projectId?: string | null
        operation?: string | null
      }
    ) => ['billing', 'ku-history', workspaceId, params] as const
  }
}
