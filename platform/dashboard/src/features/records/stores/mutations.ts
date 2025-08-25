import type { ApiParams, ApiResult } from '~/lib/api'

import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'

export const deleteRecordMutation = createMutator<
  ApiParams<typeof api.records.deleteById>,
  ApiResult<typeof api.records.deleteById>
>({
  async fetcher({ init, id }) {
    if (!id) {
      return
    }

    return await api.records.deleteById({ id, init })
  }
})
