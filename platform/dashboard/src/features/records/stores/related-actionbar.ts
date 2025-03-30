import { action, atom, computed, onSet } from 'nanostores'

import type { AnySearchOperation } from '~/features/search/types'
import type { BatchActionSelection } from '~/types'

import { toast } from '~/elements/Toast'
import {
  // $activeLabels,
  // $combineFilters,
  $currentProjectFields,
  // $currentProjectFilters,
  $currentProjectLabels,
  $filteredRecords
} from '~/features/projects/stores/current-project'
// import {
//   convertToSearchQuery,
//   filterToSearchOperation
// } from '~/features/projects/utils'
import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'

import { $currentProjectId } from '../../projects/stores/id'
import { $currentRelatedRecords } from '~/features/projects/stores/current-record.ts'

export const $selectedRelatedRecords = atom<BatchActionSelection>([])

export const $hasRelatedRecordsSelection = computed(
  $selectedRelatedRecords,
  (selectedRecords) =>
    // selectedRecords === '*' ||
    Array.isArray(selectedRecords) && selectedRecords.length > 0
)

export const $mixedRelatedRecordsSelection = computed(
  [$selectedRelatedRecords, $currentRelatedRecords],
  (selectedRecords, currentRelatedRecords) => selectedRecords.length !== currentRelatedRecords.data?.length
)

export const $selectionRelatedLength = computed(
  [$selectedRelatedRecords, $hasRelatedRecordsSelection, $mixedRelatedRecordsSelection],
  (selectedRecords, hasSelection, mixed) => {
    return selectedRecords.length
    // if (!hasSelection) {
    //   return 0
    // } else if (mixed) {
    //   return selectedRecords.length
    // } else {
    //   return $currentRelatedRecords.get().total
    // }
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

    // if (current === '*') {
    //   //  this logic is not correct
    //   store.set(
    //     $currentRelatedRecords
    //       .get()
    //       .data?.map((record) => record.__id)
    //       .filter((id) => id !== recordId) ?? []
    //   )
    // } else
    if (selected) {
      store.set(current.filter((id) => id !== recordId))
    } else {
      store.set([...current, recordId])
    }
  }
)

export const batchDeleteRelatedSelected = createMutator({
  async fetcher({ init }) {
    const mixed = $mixedRelatedRecordsSelection.get()
    let body:
      | {
          labels?: Array<string>
          where?: Array<AnySearchOperation>
        }
      | { ids: Array<string> } = {}
    // if (mixed) {

    body = {
      ids: $selectedRelatedRecords.get() as Array<string>
    }
    // } else {
    //   const filters = $currentProjectFilters.get()
    //   body['labels'] = $activeLabels.get()
    //   const combineMode = $combineFilters.get()
    //
    //   if (combineMode === 'and') {
    //     const properties = filters.map(filterToSearchOperation)
    //     // Мне нужны вэлью которых нет при and
    //     body['where'] = properties
    //   }
    // }

    return await api.records.delete({
      init,
      ...body
      // where: ('where' in body && convertToSearchQuery(body?.where)) || {}
    })
  },
  invalidates: [$currentProjectLabels, $currentProjectFields, $currentRelatedRecords],
  onSuccess: () => {
    resetRelatedRecordsSelection()
    toast({
      title: 'Records were successfully deleted'
    })
  }
})

// effects

onSet($currentProjectId, resetRelatedRecordsSelection)
