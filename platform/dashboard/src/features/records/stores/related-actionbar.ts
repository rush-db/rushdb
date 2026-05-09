import { action, atom, computed, onSet } from 'nanostores'

import type { BatchActionSelection } from '~/types'

import { $currentProjectId } from '../../projects/stores/id'

export const $selectedRelatedRecords = atom<BatchActionSelection>([])

export const $hasRelatedRecordsSelection = computed(
  $selectedRelatedRecords,
  (selectedRecords) => Array.isArray(selectedRecords) && selectedRecords.length > 0
)

export const $selectionRelatedLength = computed($selectedRelatedRecords, (selectedRecords) => {
  return selectedRecords.length
})

export const resetRelatedRecordsSelection = action(
  $selectedRelatedRecords,
  'resetRecordsSelection',
  (store) => {
    store.set([])
  }
)

export const toggleRelatedRecordSelection = action(
  $selectedRelatedRecords,
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

onSet($currentProjectId, resetRelatedRecordsSelection)
