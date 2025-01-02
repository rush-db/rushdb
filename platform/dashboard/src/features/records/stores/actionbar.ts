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
  $currentProjectRecordsSkip,
  $filteredRecords
} from '~/features/projects/stores/current-project'
// import {
//   convertToSearchQuery,
//   filterToSearchOperation
// } from '~/features/projects/utils'
import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'

import { $currentProjectId } from '../../projects/stores/id'

export const $selectedRecords = atom<BatchActionSelection>([])

export const $hasRecordsSelection = computed(
  $selectedRecords,
  (selectedRecords) =>
    // selectedRecords === '*' ||
    Array.isArray(selectedRecords) && selectedRecords.length > 0
)

export const $mixedRecordsSelection = computed(
  [$selectedRecords, $filteredRecords],
  (selectedRecords, filteredRecords) => selectedRecords.length !== filteredRecords.data?.length
)

export const $selectionLength = computed(
  [$selectedRecords, $hasRecordsSelection, $mixedRecordsSelection],
  (selectedRecords, hasSelection, mixed) => {
    return selectedRecords.length
    // if (!hasSelection) {
    //   return 0
    // } else if (mixed) {
    //   return selectedRecords.length
    // } else {
    //   return $filteredRecords.get().total
    // }
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

    // if (current === '*') {
    //   //  this logic is not correct
    //   store.set(
    //     $filteredRecords
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

export const batchDeleteSelected = createMutator({
  async fetcher({ init }) {
    // const mixed = $mixedRecordsSelection.get()
    let body:
      | {
          labels?: Array<string>

          where?: Array<AnySearchOperation>
        }
      | { ids: Array<string> } = {}
    // if (mixed) {
    body = {
      ids: $selectedRecords.get() as Array<string>
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

    return await api.records.batchDelete({
      init,
      ...body
      // where: ('where' in body && convertToSearchQuery(body?.where)) || {}
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
