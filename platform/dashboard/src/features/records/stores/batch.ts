import type { AnyRecord } from 'dns'

import {
  $currentProjectFields,
  $currentProjectLabels,
  $filteredRecords
} from '~/features/projects/stores/current-project'
import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'
import { DBRecordCreationOptions } from '@rushdb/javascript-sdk'

export const createMany = createMutator<{
  label: string
  options?: DBRecordCreationOptions
  data: AnyRecord | Array<unknown>
}>({
  async fetcher({ init, data, label, options }) {
    return await api.records.createMany({ init, data, label, options })
  },
  throwError: true,
  onError: (error: unknown) => console.log({ error }),
  invalidates: [$filteredRecords, $currentProjectLabels, $currentProjectFields]
})

export const importCsv = createMutator<{
  label: string
  data: string
  options?: DBRecordCreationOptions
  parseConfig?: {
    delimiter?: string
    header?: boolean
    skipEmptyLines?: boolean | 'greedy'
    dynamicTyping?: boolean
    quoteChar?: string
    escapeChar?: string
    newline?: string
  }
}>({
  async fetcher({ init, data, label, options, parseConfig }) {
    // The SDK method expects params object; we pass init manually for auth/context
    return await api.records.importCsv({ init, label, data, options, parseConfig })
  },
  throwError: true,
  onError: (error: unknown) => console.log({ error }),
  invalidates: [$filteredRecords, $currentProjectLabels, $currentProjectFields]
})
