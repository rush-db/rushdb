import type { SearchQuery } from '@rushdb/javascript-sdk'

import { atom } from 'nanostores'

import type { SavedQuery } from '~/features/saved-queries/types'
import { openRoute, removeSearchParam } from '~/lib/router'

import { $currentProjectId } from './id'
import { $activeLabels, $currentProjectRecordsSkip, $recordView } from './current-project'

export type RecordsSearchMode = 'manual' | 'ai' | 'semantic'

export const $recordsSearchMode = atom<RecordsSearchMode>('ai')
export const $aiSearchPrompt = atom('')
export const $aiSearchQuery = atom<SearchQuery | undefined>()
export const $draftSearchQuery = atom<SearchQuery | undefined>()
export const $semanticSearchPrompt = atom('')
export const $semanticSearchIndexId = atom<string | undefined>()
export const $searchQueryModalOpen = atom(false)

// Builder search-box input text; lives here (not in SearchBox.tsx) so it can be
// reset alongside the rest of the search state on project switch.
export const $recordQuery = atom<string>('')

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

// Clears the AI prompt and its generated query so results fall back to defaults.
// Mode is intentionally left as-is: getEffectiveRecordsSearchMode treats 'ai'
// without a query as 'manual', so the default records view is restored while the
// user stays on the Smart tab.
export const resetAiSearch = () => {
  $aiSearchPrompt.set('')
  $aiSearchQuery.set(undefined)
  $draftSearchQuery.set(undefined)
}

// Returns the Records view to its first-open defaults: no filters, no label drill-down,
// Smart (AI) mode, Table view, first page. Clearing the `query`/`view` URL params (which
// happens synchronously) also lets the subsequent tab navigation land on a clean URL.
export const resetRecordsView = () => {
  removeSearchParam('query')
  removeSearchParam('view')
  $activeLabels.set([])
  $currentProjectRecordsSkip.set(0)
  $recordsSearchMode.set('ai')
  resetAiSearch()
  $semanticSearchPrompt.set('')
  $semanticSearchIndexId.set(undefined)
}

// Search state lives in module-level atoms, so it survives client-side navigation
// between projects. Clear it whenever the current project changes; mirrors the
// filters/pagination reset in current-project.ts. Same-project navigation (e.g.
// applySavedQuery) is unaffected: the atom doesn't notify when the id is unchanged.
$currentProjectId.subscribe(() => {
  $recordsSearchMode.set('ai')
  resetAiSearch()
  $semanticSearchPrompt.set('')
  $semanticSearchIndexId.set(undefined)
  $recordQuery.set('')
  $searchQueryModalOpen.set(false)
})

// Restores a saved query onto the Records page and navigates there. Manual and
// AI queries are re-run through the AI path (which executes the stored
// SearchQuery verbatim); semantic queries restore the index + prompt so the
// semantic search path kicks in.
export const applySavedQuery = (savedQuery: SavedQuery) => {
  $currentProjectRecordsSkip.set(0)
  $recordView.set('table')

  if (savedQuery.searchMode === 'semantic') {
    $recordsSearchMode.set('semantic')
    $semanticSearchIndexId.set(savedQuery.semanticIndexId ?? undefined)
    $semanticSearchPrompt.set(savedQuery.prompt ?? '')
  } else {
    $aiSearchPrompt.set(savedQuery.searchMode === 'ai' ? (savedQuery.prompt ?? '') : '')
    setAiSearchQuery(savedQuery.searchQuery)
  }

  const projectId = $currentProjectId.get()
  if (projectId) {
    openRoute('project', { id: projectId })
  }
}
