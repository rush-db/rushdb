import type { AnyRecord } from 'dns'

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
  onError: (error: unknown) => console.log({ error })
})
