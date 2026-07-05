import type { ReactNode } from 'react'

import { useQuery } from '@tanstack/react-query'
import { Check, X } from 'lucide-react'

import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { Setting, SettingsList } from '~/elements/Setting'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import { WorkspaceSettingsLayout } from '~/features/workspaces/layout/WorkspaceSettingsLayout'
import { api } from '~/lib/api'
import { queryKeys } from '~/lib/queryKeys'

import { version } from '../../../package.json'

function InfoRow({ children, label }: { children: ReactNode; label: string }) {
  return (
    <div className="flex items-center justify-between gap-4 py-4 first:pt-0 last:pb-0">
      <span className="text-sm text-content2">{label}</span>
      <span className="text-sm font-medium text-content">{children}</span>
    </div>
  )
}

function HealthStatus() {
  const { data, isError, isLoading } = useQuery({
    queryFn: () => api.health.get({}),
    queryKey: queryKeys.health(),
    retry: false,
    staleTime: 30_000
  })

  if (isLoading) {
    return <span className="text-sm text-content3">Checking…</span>
  }

  const healthy = !isError && data?.status === 'ok'

  return healthy ?
      <span className="inline-flex items-center gap-1 rounded-md bg-success/10 px-2 py-1 text-xs font-medium text-success">
        <Check className="h-3.5 w-3.5" /> Operational
      </span>
    : <span className="inline-flex items-center gap-1 rounded-md bg-danger/10 px-2 py-1 text-xs font-medium text-danger">
        <X className="h-3.5 w-3.5" /> Unavailable
      </span>
}

export function WorkspaceSystemInfoPage() {
  const { data: platformSettings } = usePlatformSettings()

  return (
    <WorkspacesLayout>
      <WorkspaceSettingsLayout>
        <PageHeader contained>
          <div className="flex max-w-3xl flex-col gap-2">
            <PageTitle>System Info</PageTitle>
            <p className="text-sm leading-6 text-content2">
              Status and version information for this RushDB deployment. These values are read-only.
            </p>
          </div>
        </PageHeader>

        <PageContent contained>
          <SettingsList>
            <Setting readOnly description="Live health check and build information." title="System">
              <div className="divide-y">
                <InfoRow label="API health">
                  <HealthStatus />
                </InfoRow>
                <InfoRow label="Dashboard version">v{version}</InfoRow>
                <InfoRow label="Deployment">{platformSettings?.selfHosted ? 'Self-hosted' : 'Cloud'}</InfoRow>
              </div>
            </Setting>
          </SettingsList>
        </PageContent>
      </WorkspaceSettingsLayout>
    </WorkspacesLayout>
  )
}
