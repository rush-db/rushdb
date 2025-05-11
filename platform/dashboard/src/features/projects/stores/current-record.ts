import { atom } from 'nanostores'

import { api } from '~/lib/api'
import { createAsyncStore } from '~/lib/fetcher'
import { $router } from '~/lib/router'

import { $currentRecordId } from './id'
import { $sheetRecordId } from './id'

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

export const $currentRelatedRecords = createAsyncStore({
  key: '$currentRecordChildren',
  async fetcher(init) {
    const id = $sheetRecordId.get() ?? $currentRecordId.get()

    if (!id) {
      return
    }
    const { data, total } = await api.relationships.find({
      searchQuery: {
        where: { $id: id }
      },
      init
    })
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

    return await api.properties.find({
      searchQuery: {
        where: { $id: id }
      },
      init
    })
  },
  deps: [$sheetRecordId, $currentRecordId]
})

// Effects
$router.subscribe((page) => {
  $sheetRecordId.set(undefined)
})
