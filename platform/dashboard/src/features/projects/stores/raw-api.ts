import { atom } from 'nanostores'
import { api } from '~/lib/api.ts'

export const $editorData = atom<string | undefined>('')

export const $selectedOperation = atom<
  | `records.${keyof (typeof api)['records']}`
  | `labels.${keyof (typeof api)['labels']}`
  | `properties.${keyof (typeof api)['properties']}`
  | `relations.${keyof (typeof api)['relationships']}`
>('records.find')
