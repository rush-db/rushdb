import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useStore } from '@nanostores/react'

import type { SsoConfig, UpsertSsoConfigPayload } from '~/features/sso/types'

import { api } from '~/lib/api'
import { toast } from '~/elements/Toast'
import { queryKeys } from '~/lib/queryKeys'
import { $currentWorkspaceId } from '~/features/workspaces/stores/current'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { useCurrentWorkspaceQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import type { PlanId } from '~/features/billing/types'

// SSO is a premium capability: Scale/Enterprise on cloud, always available self-hosted.
// Mirrors the backend gate in BillingPolicyPort.assertSsoAllowed.
const SSO_ELIGIBLE_PLANS: ReadonlyArray<PlanId> = ['scale', 'enterprise']

export const useCanUseSso = (): boolean => {
  const { data: settings } = usePlatformSettings()
  const { data: workspace } = useCurrentWorkspaceQuery()
  if (settings?.selfHosted) return true
  return Boolean(workspace?.planId && SSO_ELIGIBLE_PLANS.includes(workspace.planId))
}

export const useSsoConfigsQuery = () => {
  const workspaceId = useStore($currentWorkspaceId)
  return useQuery({
    queryKey: workspaceId ? queryKeys.workspaces.sso(workspaceId) : ['workspaces', 'sso', 'none'],
    queryFn: () => api.sso.list({ id: workspaceId as string }),
    enabled: Boolean(workspaceId)
  })
}

export const useUpsertSsoMutation = () => {
  const queryClient = useQueryClient()
  const workspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: (payload: UpsertSsoConfigPayload) => {
      if (!workspaceId) throw new Error('No workspace selected')
      return api.sso.upsert({ id: workspaceId, payload })
    },
    onSuccess() {
      if (!workspaceId) return
      queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.sso(workspaceId) })
      toast({ title: 'SSO configuration saved', duration: 3000 })
    },
    onError(err) {
      toast({
        title: 'Could not save SSO configuration',
        description: err instanceof Error ? err.message : '',
        variant: 'danger'
      })
    }
  })
}

export const useDeleteSsoMutation = () => {
  const queryClient = useQueryClient()
  const workspaceId = useStore($currentWorkspaceId)
  return useMutation({
    mutationFn: (config: Pick<SsoConfig, 'id'>) => {
      if (!workspaceId) throw new Error('No workspace selected')
      return api.sso.remove({ id: workspaceId, configId: config.id })
    },
    onSuccess() {
      if (!workspaceId) return
      queryClient.invalidateQueries({ queryKey: queryKeys.workspaces.sso(workspaceId) })
      toast({ title: 'SSO configuration removed', duration: 3000 })
    }
  })
}
