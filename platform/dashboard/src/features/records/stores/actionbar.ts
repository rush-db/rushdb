import { action, atom, computed, onSet } from 'nanostores'

import type { BatchActionSelection } from '~/types'

import { $currentProjectRecordsSkip } from '~/features/projects/stores/current-project'

import { $currentProjectId } from '../../projects/stores/id'

export const $selectedRecords = atom<BatchActionSelection>([])

export const $hasRecordsSelection = computed(
  $selectedRecords,
  (selectedRecords) => Array.isArray(selectedRecords) && selectedRecords.length > 0
)

export const $selectionLength = computed($selectedRecords, (selectedRecords) => {
  return selectedRecords.length
})

export const resetRecordsSelection = action($selectedRecords, 'resetRecordsSelection', (store) => {
  store.set([])
})

export const toggleRecordSelection = action(
  $selectedRecords,
  'toggleRecordSelection',
  (store, { recordId, selected }: { recordId: string; selected: boolean }) => {
    const current = store.get()

    if (selected) {
      store.set(current.filter((id) => id !== recordId))
    } else {
      store.set([...current, recordId])
    }
  }
)

// effects

onSet($currentProjectId, resetRecordsSelection)

$currentProjectRecordsSkip.subscribe(() => {
  $selectedRecords.set([])
})
