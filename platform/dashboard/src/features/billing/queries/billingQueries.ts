import { queryOptions } from '@tanstack/react-query'

import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'

export const pricingDataQueryOptions = (selfHosted: boolean | undefined) =>
  queryOptions({
    queryKey: queryKeys.billing.data(),
    queryFn: () => api.billing.getBillingData(),
    enabled: !selfHosted,
    staleTime: 10 * 60 * 1000
  })

export const workspaceUsageQueryOptions = (params: {
  workspaceId: string | undefined
  selfHosted: boolean | undefined
}) =>
  queryOptions({
    queryKey: queryKeys.billing.usage(params.workspaceId!),
    queryFn: () => api.billing.getUsage(),
    enabled: !!params.workspaceId && !params.selfHosted
  })

export const kuHistoryQueryOptions = (params: {
  workspaceId: string | undefined
  selfHosted: boolean | undefined
  limit?: number
  before?: string
  since?: string
  projectId?: string | null
  operation?: string | null
}) =>
  queryOptions({
    queryKey: queryKeys.billing.kuHistory(params.workspaceId!, {
      limit: params.limit,
      before: params.before,
      since: params.since,
      projectId: params.projectId,
      operation: params.operation
    }),
    queryFn: () =>
      api.billing.getKuHistory({
        limit: params.limit,
        before: params.before,
        since: params.since,
        projectId: params.projectId,
        operation: params.operation
      }),
    enabled: !!params.workspaceId && !params.selfHosted
  })
