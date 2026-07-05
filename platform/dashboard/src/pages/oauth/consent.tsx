import { useStore } from '@nanostores/react'
import { useEffect, useState } from 'react'

import { Button } from '~/elements/Button'
import { Card, CardBody, CardFooter, CardHeader } from '~/elements/Card'
import { $user } from '~/features/auth/stores/user'
import type { Project } from '~/features/projects/types'
import type { Workspace } from '~/features/workspaces/types'
import { $currentWorkspaceId } from '~/features/workspaces/stores/current'
import { api } from '~/lib/api'
import { $searchParams, openRoute } from '~/lib/router'

type AuthRequestInfo = {
  auth_request_id: string
  client_name: string
  scope: string
  resource: string
  expires_at: string
}

const SCOPE_META: Record<string, { label: string; description: string }> = {
  'records:read': { label: 'Read records', description: 'View your project data and records' },
  'records:write': { label: 'Read & write records', description: 'Create, update and delete project data' }
}

export function OAuthConsentPage() {
  const searchParams = useStore($searchParams)
  const currentUser = useStore($user)

  const requestId = (searchParams?.request_id ?? searchParams?.auth_request_id) as string | undefined

  const [authRequest, setAuthRequest] = useState<AuthRequestInfo | null>(null)
  const [workspaces, setWorkspaces] = useState<Workspace[]>([])
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<string | null>(null)
  const [projects, setProjects] = useState<Project[]>([])
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null)
  const [selectedScopes, setSelectedScopes] = useState<Set<string>>(new Set())
  const [loading, setLoading] = useState(false)
  const [fetchError, setFetchError] = useState<string | null>(null)
  const [acting, setActing] = useState(false)

  // Redirect to sign-in if not logged in
  useEffect(() => {
    if (!currentUser.isLoggedIn) {
      const returnUrl = encodeURIComponent(window.location.pathname + window.location.search)
      openRoute('signin')
      window.history.replaceState(null, '', `/signin?return=${returnUrl}`)
    }
  }, [currentUser.isLoggedIn])

  // Load auth request info and workspaces on mount
  useEffect(() => {
    if (!requestId || !currentUser.isLoggedIn) return

    setLoading(true)
    Promise.all([api.oauth.getAuthRequest(requestId), api.workspaces.list({})])
      .then(([info, wsList]) => {
        setAuthRequest(info)
        setWorkspaces(wsList)
        setSelectedScopes(new Set((info.scope || '').split(' ').filter(Boolean)))

        const currentWsId = $currentWorkspaceId.get()
        const initialWs = wsList.find((w) => w.id === currentWsId) ?? wsList[0]
        const initialWsId = initialWs?.id ?? null
        setSelectedWorkspaceId(initialWsId)
      })
      .catch(() => setFetchError('Authorization request not found or expired.'))
      .finally(() => setLoading(false))
  }, [requestId, currentUser.isLoggedIn])

  // Load projects when workspace selection changes
  useEffect(() => {
    if (!selectedWorkspaceId) {
      setProjects([])
      setSelectedProjectId(null)
      return
    }

    api.projects
      .list({ headers: { 'x-workspace-id': selectedWorkspaceId } } as RequestInit)
      .then(({ data }) => {
        setProjects(data)
        setSelectedProjectId(null)
      })
      .catch(() => setProjects([]))
  }, [selectedWorkspaceId])

  const toggleScope = (scope: string) => {
    setSelectedScopes((prev) => {
      const next = new Set(prev)
      if (next.has(scope)) {
        if (next.size > 1) next.delete(scope) // keep at least one
      } else {
        next.add(scope)
      }
      return next
    })
  }

  const handleAllow = async () => {
    if (!authRequest) return
    if (!selectedProjectId) {
      setFetchError('Please select a project to connect.')
      return
    }
    if (selectedScopes.size === 0) {
      setFetchError('Please select at least one permission.')
      return
    }
    setActing(true)
    try {
      const result = await api.oauth.acceptAuthorization({
        authRequestId: authRequest.auth_request_id,
        projectId: selectedProjectId,
        scope: Array.from(selectedScopes).join(' ')
      })
      if (result?.redirectTo) {
        window.location.href = result.redirectTo
      }
    } catch {
      setFetchError('Failed to authorize. Please try again.')
    } finally {
      setActing(false)
    }
  }

  const handleDeny = async () => {
    if (!authRequest) return
    setActing(true)
    try {
      const result = await api.oauth.denyAuthorization(authRequest.auth_request_id)
      if (result?.redirectTo) {
        window.location.href = result.redirectTo
      }
    } catch {
      openRoute('home')
    } finally {
      setActing(false)
    }
  }

  if (!currentUser.isLoggedIn) {
    return null
  }

  const requestedScopes = authRequest?.scope.split(' ').filter(Boolean) ?? []
  const canAllow = selectedProjectId !== null && selectedScopes.size > 0

  return (
    <div className="bg-background flex min-h-screen items-center justify-center p-4">
      <div className="w-full max-w-md">
        {loading && (
          <Card>
            <CardBody className="text-content-2 py-10 text-center">Loading authorization request…</CardBody>
          </Card>
        )}

        {fetchError && !loading && (
          <Card>
            <CardHeader title="Authorization Error" />
            <CardBody>
              <p className="text-sm text-danger">{fetchError}</p>
            </CardBody>
            <CardFooter>
              <Button onClick={() => openRoute('home')} variant="secondary">
                Go to Dashboard
              </Button>
            </CardFooter>
          </Card>
        )}

        {authRequest && !loading && !fetchError && (
          <Card>
            <CardHeader
              title={`Authorize ${authRequest.client_name}`}
              description={`${authRequest.client_name} is requesting access to your RushDB data.`}
            />
            <CardBody className="gap-4">
              {/* Permissions — checkboxes */}
              <div>
                <p className="text-content-2 mb-2 text-xs font-semibold tracking-wide uppercase">
                  Permissions to grant
                </p>
                <ul className="space-y-1 rounded-md border bg-secondary p-3">
                  {requestedScopes.map((scope) => {
                    const meta = SCOPE_META[scope] ?? { label: scope, description: '' }
                    const checked = selectedScopes.has(scope)
                    return (
                      <li key={scope} className="flex items-start gap-2 py-1">
                        <input
                          checked={checked}
                          className="mt-0.5 h-4 w-4 cursor-pointer accent-accent"
                          id={`scope-${scope}`}
                          onChange={() => toggleScope(scope)}
                          type="checkbox"
                        />
                        <label className="cursor-pointer" htmlFor={`scope-${scope}`}>
                          <p className="text-sm font-medium text-content">{meta.label}</p>
                          {meta.description && <p className="text-content-2 text-xs">{meta.description}</p>}
                        </label>
                      </li>
                    )
                  })}
                </ul>
              </div>

              {/* Workspace selector — only shown when there are multiple workspaces */}
              {workspaces.length > 1 && (
                <div>
                  <p className="text-content-2 mb-2 text-xs font-semibold tracking-wide uppercase">
                    Select a workspace
                  </p>
                  <ul className="space-y-1 rounded-md border bg-secondary p-3">
                    {workspaces.map((ws) => (
                      <li key={ws.id} className="flex items-center gap-2 py-1">
                        <input
                          checked={selectedWorkspaceId === ws.id}
                          className="h-4 w-4 cursor-pointer accent-accent"
                          id={`ws-${ws.id}`}
                          name="workspace"
                          onChange={() => setSelectedWorkspaceId(ws.id)}
                          type="radio"
                        />
                        <label className="cursor-pointer text-sm text-content" htmlFor={`ws-${ws.id}`}>
                          {ws.name}
                        </label>
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {/* Project selector */}
              {projects.length > 0 && (
                <div>
                  <p className="text-content-2 mb-2 text-xs font-semibold tracking-wide uppercase">
                    Select a project to connect
                  </p>
                  <ul className="space-y-1 rounded-md border bg-secondary p-3">
                    {projects.map((project) => (
                      <li key={project.id} className="flex items-center gap-2 py-1">
                        <input
                          checked={selectedProjectId === project.id}
                          className="h-4 w-4 cursor-pointer accent-accent"
                          id={`proj-${project.id}`}
                          name="project"
                          onChange={() => setSelectedProjectId(project.id)}
                          type="radio"
                        />
                        <label className="cursor-pointer text-sm text-content" htmlFor={`proj-${project.id}`}>
                          {project.name}
                        </label>
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {selectedWorkspaceId && projects.length === 0 && (
                <p className="text-content-2 text-xs">No projects found in this workspace.</p>
              )}

              <p className="text-content-2 text-xs">
                Authorizing will create a limited-access API token on your behalf. You can revoke access at
                any time from{' '}
                <button
                  className="underline hover:text-content"
                  onClick={() => openRoute('workspaceSettings')}
                >
                  Workspace Settings
                </button>
                .
              </p>
            </CardBody>
            <CardFooter className="justify-between">
              <Button disabled={acting} onClick={handleDeny} variant="secondary">
                Deny
              </Button>
              <Button disabled={acting || !canAllow} loading={acting} onClick={handleAllow} variant="primary">
                Allow Access
              </Button>
            </CardFooter>
          </Card>
        )}
      </div>
    </div>
  )
}
