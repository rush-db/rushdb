import type { AnyRecord } from 'dns'

import {
  $currentProject,
  $currentProjectFields,
  $currentProjectLabels
} from '~/features/projects/stores/current-project'
import { $workspaceProjects } from '~/features/workspaces/stores/projects'
import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'
import { DBRecordCreationOptions } from '@rushdb/javascript-sdk'

export const importJson = createMutator<{
  label: string
  options?: DBRecordCreationOptions
  data: AnyRecord | Array<unknown>
}>({
  async fetcher({ init, data, label, options }) {
    return await api.records.importJson({ init, data, label, options })
  },
  throwError: true,
  onError: (error: unknown) => console.log({ error }),
  invalidates: [$currentProjectLabels, $currentProjectFields, $workspaceProjects, $currentProject]
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
  invalidates: [$currentProjectLabels, $currentProjectFields, $workspaceProjects, $currentProject]
})
