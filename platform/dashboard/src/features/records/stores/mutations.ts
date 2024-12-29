import type { ApiParams, ApiResult } from '~/lib/api'

import {
  $currentProjectFields,
  $currentProjectLabels,
  $filteredRecords
} from '~/features/projects/stores/current-project'
import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'

export const deleteRecordMutation = createMutator<
  ApiParams<typeof api.records.deleteById>,
  ApiResult<typeof api.records.deleteById>
>({
  invalidates: [$currentProjectLabels, $currentProjectFields, $filteredRecords],
  async fetcher({ init, id }) {
    if (!id) {
      return
    }

    return await api.records.deleteById({ id, init })
  }
})
