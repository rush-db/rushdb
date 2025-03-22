import { createMutator } from '~/lib/fetcher.ts'
import { api } from '~/lib/api.ts'

import { SearchQuery } from '@rushdb/javascript-sdk'
import { atom } from 'nanostores'

export const $editorData = atom<string | undefined>('')

export const rawRecords = createMutator<{
  searchQuery: SearchQuery
}>({
  async fetcher({ init, searchQuery }) {
    return await api.records.find(searchQuery, init)
  },
  throwError: true,
  onError: (error: unknown) => console.log({ error }),
  invalidates: []
})

export const rawLabels = createMutator<{
  searchQuery: SearchQuery
}>({
  async fetcher({ init, searchQuery }) {
    return await api.records.labels({ searchQuery, init })
  },
  throwError: true,
  onError: (error: unknown) => console.log({ error }),
  invalidates: []
})

export const rawProperties = createMutator<{
  searchQuery: SearchQuery
}>({
  async fetcher({ init, searchQuery }) {
    return await api.properties.list(searchQuery, init)
  },
  throwError: true,
  onError: (error: unknown) => console.log({ error }),
  invalidates: []
})
