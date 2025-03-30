import type { AnyRecord } from 'dns'

import {
  $currentProjectFields,
  $currentProjectLabels,
  $filteredRecords
} from '~/features/projects/stores/current-project'
import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'

export const createMany = createMutator<{
  label: string
  options?: {
    suggestTypes?: boolean
    castNumberArraysAsVector?: boolean
    castNumericValuesAsNumber?: boolean
  }
  payload: AnyRecord | Array<unknown>
}>({
  async fetcher({ init, payload, label, options }) {
    return await api.records.createMany({ init, payload, label, options })
  },
  throwError: true,
  onError: (error: unknown) => console.log({ error }),
  invalidates: [$filteredRecords, $currentProjectLabels, $currentProjectFields]
})
