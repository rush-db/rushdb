import RushDB, { HttpClient, HttpClientResponse } from '@rushdb/javascript-sdk'

import { BASE_URL } from '~/config.ts'
import { $token } from '~/features/auth/stores/token.ts'
import { $currentProjectId } from '~/features/projects/stores/id.ts'
import { $currentWorkspaceId } from '~/features/workspaces/stores/current.ts'

class CustomHttpClientResponse extends HttpClientResponse {
  _res: Response
  constructor(response: Response) {
    super(response)
    this._res = response
  }

  getStatusCode() {
    return this._res.status
  }

  toJSON() {
    return this._res.json()
  }
}

class CustomHttpClient extends HttpClient {
  constructor() {
    super()
  }

  private getDynamicHeaders(): HeadersInit {
    const token = $token.get()
    const currentWorkspaceId = $currentWorkspaceId.get()
    const currentProjectId = $currentProjectId.get()

    return token ?
        {
          Authorization: `Bearer ${token}`,
          ...(currentWorkspaceId ? { 'x-workspace-id': currentWorkspaceId } : {}),
          ...(currentProjectId ? { 'x-project-id': currentProjectId } : {})
        }
      : {}
  }

  async makeRequest(url: string, { ...init }) {
    const dynamicHeaders = this.getDynamicHeaders()

    const res = await fetch(url, {
      ...init,
      body: JSON.stringify(init.requestData),
      signal: init.signal, // Use signal from the request init if provided
      headers: {
        ...init.headers,
        ...dynamicHeaders
      }
    })

    return new CustomHttpClientResponse(res)
  }
}

// Create a singleton RushDB instance
export const rushDBInstance = new RushDB(undefined, {
  url: BASE_URL || document.URL,
  httpClient: new CustomHttpClient()
})
