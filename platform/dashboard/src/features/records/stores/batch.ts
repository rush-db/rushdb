import type { AnyRecord } from 'dns'

import {
  $currentProjectFields,
  $currentProjectLabels,
  $filteredRecords
} from '~/features/projects/stores/current-project'
import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'
import { DBRecordWriteOptions } from '@rushdb/javascript-sdk'

export const createMany = createMutator<{
  label: string
  options?: DBRecordWriteOptions
  payload: AnyRecord | Array<unknown>
}>({
  async fetcher({ init, payload, label, options }) {
    return await api.records.createMany({ init, payload, label, options })
  },
  throwError: true,
  onError: (error: unknown) => console.log({ error }),
  invalidates: [$filteredRecords, $currentProjectLabels, $currentProjectFields]
})
