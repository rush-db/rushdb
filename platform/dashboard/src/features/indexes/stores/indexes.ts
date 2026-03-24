import type { ApiParams } from '~/lib/api'

import { $currentProjectIndexes } from '~/features/projects/stores/current-project'
import { $currentProjectId } from '~/features/projects/stores/id'
import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'

import type { EmbeddingIndex } from '../types'

export const addIndex = createMutator({
  async fetcher(args: ApiParams<typeof api.indexes.create>) {
    return api.indexes.create(args)
  },
  invalidates: [$currentProjectIndexes]
})

export const deleteIndex = createMutator<{
  indexId: EmbeddingIndex['id']
}>({
  async fetcher({ indexId, init }) {
    return api.indexes.delete({ init, id: indexId })
  },
  invalidates: [$currentProjectIndexes]
})
