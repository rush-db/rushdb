import type { SearchQuery } from '@rushdb/javascript-sdk'

import type { RecordsSearchMode } from '~/features/projects/stores/records-search'

export type SavedQuery = {
  id: string
  projectId: string
  name: string
  searchMode: RecordsSearchMode
  prompt?: string | null
  searchQuery: SearchQuery
  semanticIndexId?: string | null
  createdBy?: string | null
  createdAt: string
  updatedAt: string
}

export type CreateSavedQueryInput = {
  name: string
  searchMode: RecordsSearchMode
  searchQuery: SearchQuery
  prompt?: string
  semanticIndexId?: string
}
