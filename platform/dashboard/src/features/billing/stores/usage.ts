import { createAsyncStore } from '~/lib/fetcher'
import { api } from '~/lib/api'
import { $platformSettings } from '~/features/auth/stores/settings'
import { $currentWorkspaceId } from '~/features/workspaces/stores/current'

export type WorkspaceUsage = {
  plan: string
  kuConsumed: number
  kuLimit: number | null
  kuIncluded: number | null
  remaining: number | null
  billingModel: 'fixed' | 'overage' | 'usage'
  billingPeriodStart: string
}

/**
 * Shared usage store consumed by KuHeaderBar and KuLimitBanner.
 * Returns undefined for self-hosted instances (commercial billing disabled).
 */
export const $workspaceUsage = createAsyncStore({
  key: '$workspaceUsage',
  skip: () => Boolean($platformSettings.get().data?.selfHosted),
  async fetcher() {
    return api.billing.getUsage()
  },
  deps: [$currentWorkspaceId],
  mustHaveDeps: [$platformSettings]
})
