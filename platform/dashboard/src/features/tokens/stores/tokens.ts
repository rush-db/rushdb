import type { ApiParams } from '~/lib/api'

import { $currentProjectTokens } from '~/features/projects/stores/current-project'
import { api } from '~/lib/api'
import { createMutator } from '~/lib/fetcher'

import type { ProjectToken } from '../types'

export const addToken = createMutator({
  async fetcher(args: ApiParams<typeof api.tokens.create>) {
    return api.tokens.create(args)
  },
  invalidates: [$currentProjectTokens]
})

export const deleteToken = createMutator<{
  tokenId: ProjectToken['id']
}>({
  async fetcher({ tokenId, init }) {
    return api.tokens.delete({ init, id: tokenId })
  },
  invalidates: [$currentProjectTokens]
})
