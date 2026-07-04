/**
 * Provisions a throwaway project + API key against a running platform instance
 * (self-hosted mode auto-creates the admin user and Default Workspace).
 */
export const provisionApiKey = async ({
  baseUrl,
  login,
  password,
  projectName,
  customDb
}: {
  baseUrl: string
  login: string
  password: string
  projectName: string
  /** Attach the project to an external Neo4j (enables e.g. the raw-query API). */
  customDb?: { url: string; username: string; password: string }
}): Promise<{ apiKey: string; projectId: string }> => {
  const api = async (
    path: string,
    {
      method = 'GET',
      token,
      headers = {},
      body
    }: { method?: string; token?: string; headers?: Record<string, string>; body?: unknown } = {}
  ) => {
    const response = await fetch(`${baseUrl}/api/v1${path}`, {
      method,
      headers: {
        'Content-Type': 'application/json',
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
        ...headers
      },
      ...(body !== undefined ? { body: JSON.stringify(body) } : {})
    })
    const json: any = await response.json().catch(() => ({}))
    if (!response.ok) {
      throw new Error(`${method} ${path} -> ${response.status}: ${JSON.stringify(json)}`)
    }
    return json.data ?? json
  }

  const user = await api('/auth/login', { method: 'POST', body: { login, password } })
  const workspaces = await api('/workspaces', { token: user.token })
  const workspaceId: string = workspaces[0]?.id
  if (!workspaceId) {
    throw new Error('No workspace available for e2e bootstrap')
  }

  const project = await api('/projects', {
    method: 'POST',
    token: user.token,
    headers: { 'x-workspace-id': workspaceId },
    body: {
      name: projectName,
      description: 'e2e harness project',
      ...(customDb ? { customDb } : {})
    }
  })

  const token = await api('/tokens', {
    method: 'POST',
    token: user.token,
    headers: { 'x-project-id': project.id, 'x-workspace-id': workspaceId },
    body: { name: 'e2e', expiration: '1d' }
  })

  return { apiKey: token.value, projectId: project.id }
}
