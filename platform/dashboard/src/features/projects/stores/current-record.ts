import { atom } from 'nanostores'

import { DEFAULT_LIMIT } from '~/config'
import { api } from '~/lib/api'
import { createAsyncStore } from '~/lib/fetcher'
import { $router } from '~/lib/router'

import { $currentProjectId, $currentRecordId } from './id'
import { $sheetRecordId } from './id'
import { $currentProjectFilters } from '~/features/projects/stores/current-project.ts'

// stores

export const $currentRecordChildrenLimit = atom<number>(DEFAULT_LIMIT)
export const $currentRecordChildrenSkip = atom<number>(0)

export const $currentRecord = createAsyncStore({
  key: '$currentRecord',
  deps: [$sheetRecordId, $currentRecordId],
  async fetcher(init) {
    const id = $sheetRecordId.get() ?? $currentRecordId.get()

    if (!id) {
      $currentRecord.setKey('data', undefined)
      return
    }
    const result = await api.records.findById({ id, init })
    return result.data
  }
})

export const $currentRecordRelations = createAsyncStore({
  key: '$currentRecordRelations',
  deps: [$sheetRecordId, $currentRecordId],
  async fetcher(init) {
    const id = $sheetRecordId.get() ?? $currentRecordId.get()

    if (!id) {
      return
    }

    return await api.records.relations({
      id,
      init
    })
  }
})

export const $currentRelatedRecords = createAsyncStore({
  key: '$currentRecordChildren',
  async fetcher(init) {
    const id = $sheetRecordId.get() ?? $currentRecordId.get()

    if (!id) {
      return
    }
    const { data, total } = await api.records.relations({ id, init })
    return { data, total }
  },
  deps: [$sheetRecordId, $currentRecordId, $currentRecordChildrenSkip]
})

export const $currentRecordFields = createAsyncStore({
  key: '$currentRecord',
  async fetcher(init) {
    const id = $sheetRecordId.get() ?? $currentRecordId.get()

    if (!id) {
      return
    }

    return await api.records.properties(id, init)
  },
  deps: [$sheetRecordId, $currentRecordId]
})

// Effects
$router.subscribe((page) => {
  $sheetRecordId.set(undefined)
})
