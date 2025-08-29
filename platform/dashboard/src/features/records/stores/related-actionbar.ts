import { action, atom, computed, onSet } from 'nanostores'

import type { AnySearchOperation } from '~/features/search/types'
import type { BatchActionSelection } from '~/types'

import { toast } from '~/elements/Toast'
import { $currentProjectFields, $currentProjectLabels } from '~/features/projects/stores/current-project'

import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'

import { $currentProjectId } from '../../projects/stores/id'
import { $currentRelatedRecords } from '~/features/projects/stores/current-record.ts'

export const $selectedRelatedRecords = atom<BatchActionSelection>([])

export const $hasRelatedRecordsSelection = computed(
  $selectedRelatedRecords,
  (selectedRecords) => Array.isArray(selectedRecords) && selectedRecords.length > 0
)

export const $mixedRelatedRecordsSelection = computed(
  [$selectedRelatedRecords, $currentRelatedRecords],
  (selectedRecords, currentRelatedRecords) => selectedRecords.length !== currentRelatedRecords.data?.length
)

export const $selectionRelatedLength = computed(
  [$selectedRelatedRecords, $hasRelatedRecordsSelection, $mixedRelatedRecordsSelection],
  (selectedRecords, hasSelection, mixed) => {
    return selectedRecords.length
  }
)

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

export const batchDeleteRelatedSelected = createMutator({
  async fetcher({ init }) {
    let body:
      | {
          labels?: Array<string>
          where?: Array<AnySearchOperation>
        }
      | { ids: Array<string> } = {}

    body = {
      ids: $selectedRelatedRecords.get() as Array<string>
    }

    return await api.records.delete({
      init,
      ...body
    })
  },
  invalidates: [$currentRelatedRecords],
  onSuccess: () => {
    resetRelatedRecordsSelection()
    toast({
      title: 'Records were successfully deleted'
    })
  }
})

// effects

onSet($currentProjectId, resetRelatedRecordsSelection)
