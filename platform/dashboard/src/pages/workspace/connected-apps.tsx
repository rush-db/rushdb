import { Cable, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useStore } from '@nanostores/react'

import { Button } from '~/elements/Button'
import { ConfirmDialog } from '~/elements/ConfirmDialog'
import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import { WorkspaceSettingsLayout } from '~/features/workspaces/layout/WorkspaceSettingsLayout'
import { api } from '~/lib/api'
import { $currentWorkspaceId } from '~/features/workspaces/stores/current'

type Consent = {
  id: string
  client_id: string
  client_name: string
  scope: string
  project_id: string
  project_name: string
  resource: string
  created: string
}

function ScopePill({ scope }: { scope: string }) {
  const short: Record<string, string> = {
    'records:read': 'read',
    'records:write': 'read + write'
  }
  return (
    <span className="text-content-2 inline-flex items-center rounded-full border px-2 py-0.5 text-xs">
      {short[scope] ?? scope}
    </span>
  )
}

function ConsentRow({
  consent,
  onRevoke,
  showProject = true
}: {
  consent: Consent
  onRevoke: () => Promise<void>
  showProject?: boolean
}) {
  const scopes = consent.scope.split(' ').filter(Boolean)
  return (
    <li className="flex items-center justify-between gap-4 border-b px-4 py-3 last:border-b-0">
      <div className="flex min-w-0 items-center gap-3">
        <Cable className="text-content-2 h-5 w-5 shrink-0" />
        <div className="min-w-0">
          <p className="truncate text-sm font-medium text-content">{consent.client_name}</p>
          {showProject && <p className="text-content-2 truncate text-xs">Project: {consent.project_name}</p>}
          <div className="mt-1 flex flex-wrap gap-1">
            {scopes.map((s) => (
              <ScopePill key={s} scope={s} />
            ))}
          </div>
        </div>
      </div>
      <ConfirmDialog
        title={`Revoke access for ${consent.client_name}?`}
        description="This will immediately invalidate the access token issued to this application. You can re-authorize it later."
        handler={onRevoke}
        trigger={
          <Button size="small" variant="dangerGhost">
            <Trash2 />
            Revoke
          </Button>
        }
      />
    </li>
  )
}

export function ConnectionsList({
  emptyMessage = 'No connected applications yet.',
  projectId,
  showHeading = true,
  showProject = true
}: {
  emptyMessage?: string
  projectId?: string
  showHeading?: boolean
  showProject?: boolean
}) {
  const workspaceId = useStore($currentWorkspaceId)
  const [consents, setConsents] = useState<Consent[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    setLoading(true)
    setError(null)
    api.oauth
      .listConsents()
      .then((data) =>
        setConsents(
          Array.isArray(data) ?
            projectId ? data.filter((consent) => consent.project_id === projectId)
            : data
          : []
        )
      )
      .catch(() => setError('Failed to load connected applications.'))
      .finally(() => setLoading(false))
  }, [projectId, workspaceId])

  const handleRevoke = async (id: string) => {
    try {
      await api.oauth.revokeConsent(id)
      setConsents((prev) => prev.filter((c) => c.id !== id))
    } catch {
      setError('Failed to revoke access. Please try again.')
    }
  }

  return (
    <div>
      {showHeading && <h2 className="mb-4 text-base font-semibold text-content">Connected Applications</h2>}

      {loading && <p className="text-content-2 text-sm">Loading…</p>}
      {error && <p className="text-sm text-danger">{error}</p>}

      {!loading && !error && consents.length === 0 && (
        <div className="rounded-md border border-dashed p-6 text-center">
          <Cable className="text-content-2 mx-auto mb-2 h-6 w-6" />
          <p className="text-content-2 text-sm">{emptyMessage}</p>
        </div>
      )}

      {!loading && consents.length > 0 && (
        <div className="overflow-hidden rounded-md border">
          <ul>
            {consents.map((consent) => (
              <ConsentRow
                key={consent.id}
                consent={consent}
                onRevoke={() => handleRevoke(consent.id)}
                showProject={showProject}
              />
            ))}
          </ul>
        </div>
      )}
    </div>
  )
}

export function WorkspaceConnectedAppsPage() {
  return (
    <WorkspacesLayout>
      <WorkspaceSettingsLayout>
        <PageHeader contained>
          <div className="flex max-w-3xl flex-col gap-2">
            <PageTitle>Connected Apps</PageTitle>
            <p className="text-sm leading-6 text-content2">
              Review third-party applications authorized against projects in this workspace. Revoking an app
              immediately invalidates its issued token.
            </p>
          </div>
        </PageHeader>
        <PageContent contained>
          <ConnectionsList />
        </PageContent>
      </WorkspaceSettingsLayout>
    </WorkspacesLayout>
  )
}
