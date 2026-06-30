import type { SavedQueryRow } from '@/database/sql/schema/types'

export const SAVED_QUERY_SEARCH_MODES = ['manual', 'ai', 'semantic'] as const

export type SavedQuerySearchMode = (typeof SAVED_QUERY_SEARCH_MODES)[number]

/**
 * A saved query as returned by the API: identical to the stored row except the
 * `searchQuery` text column is parsed back into an object.
 */
export type PublicSavedQuery = Omit<SavedQueryRow, 'searchQuery'> & {
  searchQuery: Record<string, unknown>
}
