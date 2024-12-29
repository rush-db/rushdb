import CollectSDK, {
  HttpClient,
  HttpClientResponse
} from '@collect.so/javascript-sdk'

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
  headers: HeadersInit = {}

  constructor(
    headers: HeadersInit = {} as HeadersInit,
    signal?: AbortSignal | null
  ) {
    super()
    this.headers = headers
  }

  async makeRequest(url: string, { ...init }) {
    const res = await fetch(url, {
      ...init,
      body: JSON.stringify(init.requestData),
      headers: {
        ...init.headers,
        ...this.headers
      }
    })

    return new CustomHttpClientResponse(res)
  }
}

export const sdk = (init?: Partial<RequestInit>) => {
  const token = $token.get()
  const currentWorkspaceId = $currentWorkspaceId.get()
  const currentProjectId = $currentProjectId.get()

  const headers: HeadersInit = token
    ? {
        Authorization: `Bearer ${token}`,
        ...(currentWorkspaceId ? { 'x-workspace-id': currentWorkspaceId } : {}),
        ...(currentProjectId ? { 'x-project-id': currentProjectId } : {})
      }
    : {}

  return new CollectSDK(undefined, {
    url: BASE_URL,
    httpClient: new CustomHttpClient(headers, init?.signal)
  })
}
