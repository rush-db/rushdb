import { createMutator } from '~/lib/fetcher.ts'
import { api } from '~/lib/api.ts'

import { SearchQuery } from '@rushdb/javascript-sdk'
import { atom } from 'nanostores'
import { toast } from '~/elements/Toast.tsx'

export const $editorData = atom<string | undefined>('')

export const $selectedOperation = atom<
  | `records.${keyof (typeof api)['records']}`
  | `labels.${keyof (typeof api)['labels']}`
  | `properties.${keyof (typeof api)['properties']}`
  | `relations.${keyof (typeof api)['relationships']}`
>('records.find')

export const rawRecords = createMutator<{
  searchQuery: SearchQuery
}>({
  async fetcher({ init, searchQuery }) {
    const operation = $selectedOperation.get().split('.')
    // @ts-ignore
    return await (api?.[operation?.[0]]?.[operation?.[1]] ?? api.records.find)(searchQuery, init)
  },
  throwError: true,
  onError: (error: unknown) => {
    console.log({ error, op: $selectedOperation.get() })
    // @ts-ignore
    if (error?.message) {
      toast({
        // @ts-ignore
        title: error?.name as string,
        // @ts-ignore
        description: error?.message as string
      })
    }
  },
  invalidates: []
})

export const rawLabels = createMutator<{
  searchQuery: SearchQuery
}>({
  async fetcher({ init, searchQuery }) {
    return await api.labels.find({ searchQuery, init })
  },
  throwError: true,
  onError: (error: unknown) => console.log({ error }),
  invalidates: []
})

export const rawProperties = createMutator<{
  searchQuery: SearchQuery
}>({
  async fetcher({ init, searchQuery }) {
    return await api.properties.find({ searchQuery, init })
  },
  throwError: true,
  onError: (error: unknown) => console.log({ error }),
  invalidates: []
})
