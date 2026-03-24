import { queryOptions } from '@tanstack/react-query'

import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'

export const platformSettingsQueryOptions = queryOptions({
  queryKey: queryKeys.settings(),
  queryFn: () => api.settings.get({}),
  staleTime: 10 * 60 * 1000
})

export const currentUserQueryOptions = (token: string | undefined) =>
  queryOptions({
    queryKey: queryKeys.user(),
    queryFn: () => api.user.current(),
    enabled: !!token,
    staleTime: 5 * 60 * 1000
  })
