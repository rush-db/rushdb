import type { SearchQuery } from '@rushdb/javascript-sdk'

import { atom } from 'nanostores'

export type RecordsSearchMode = 'manual' | 'ai' | 'semantic'

export const $recordsSearchMode = atom<RecordsSearchMode>('manual')
export const $aiSearchPrompt = atom('')
export const $aiSearchQuery = atom<SearchQuery | undefined>()
export const $draftSearchQuery = atom<SearchQuery | undefined>()
export const $semanticSearchPrompt = atom('')
export const $semanticSearchIndexId = atom<string | undefined>()
export const $searchQueryModalOpen = atom(false)

export const setAiSearchQuery = (query: SearchQuery | undefined) => {
  $aiSearchQuery.set(query)
  if (query) {
    $recordsSearchMode.set('ai')
  }
}

export const setDraftSearchQuery = (query: SearchQuery | undefined) => {
  $draftSearchQuery.set(query)
  if (query) {
    $recordsSearchMode.set('ai')
  }
}
