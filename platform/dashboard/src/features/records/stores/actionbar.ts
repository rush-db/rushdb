import { action, atom, computed, onSet } from 'nanostores'

import type { AnySearchOperation } from '~/features/search/types'
import type { BatchActionSelection } from '~/types'

import { toast } from '~/elements/Toast'
import {
  $currentProjectFields,
  $currentProjectLabels,
  $currentProjectRecordsSkip,
  $filteredRecords
} from '~/features/projects/stores/current-project'

import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'

import { $currentProjectId } from '../../projects/stores/id'

export const $selectedRecords = atom<BatchActionSelection>([])

export const $hasRecordsSelection = computed(
  $selectedRecords,
  (selectedRecords) => Array.isArray(selectedRecords) && selectedRecords.length > 0
)

export const $mixedRecordsSelection = computed(
  [$selectedRecords, $filteredRecords],
  (selectedRecords, filteredRecords) => selectedRecords.length !== filteredRecords.data?.length
)

export const $selectionLength = computed(
  [$selectedRecords, $hasRecordsSelection, $mixedRecordsSelection],
  (selectedRecords, hasSelection, mixed) => {
    return selectedRecords.length
  }
)

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

export const batchDeleteSelected = createMutator({
  async fetcher({ init }) {
    let body:
      | {
          labels?: Array<string>

          where?: Array<AnySearchOperation>
        }
      | { ids: Array<string> } = {}
    body = {
      ids: $selectedRecords.get() as Array<string>
    }

    return await api.records.delete({
      init,
      ...body
    })
  },
  invalidates: [$currentProjectLabels, $currentProjectFields, $filteredRecords],
  onSuccess: () => {
    resetRecordsSelection()
    toast({
      title: 'Records were successfully deleted'
    })
  }
})

// effects

onSet($currentProjectId, resetRecordsSelection)

$currentProjectRecordsSkip.subscribe(() => {
  $selectedRecords.set([])
})
