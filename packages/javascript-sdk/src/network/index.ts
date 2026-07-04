import type { HttpClient, MakeRequestConfig } from './HttpClient.js'
import type { RequestHeaders } from './types.js'

const defaultHeaders = {
  'Content-Type': 'application/json'
}

export const createFetcher =
  ({ httpClient, token, url }: { httpClient: HttpClient; token?: string; url: string }) =>
  async <Data extends Record<string, any> = Record<string, any>>(
    input: URL | string,
    { headers: initHeaders, ...init }: MakeRequestConfig<Data>
  ): Promise<Data> => {
    const response = await httpClient.makeRequest(`${url}${input}`, {
      credentials: 'omit',
      headers: Object.assign(
        {
          ...defaultHeaders,
          ...initHeaders
        },
        typeof token !== 'undefined' ? { Authorization: `Bearer ${token}` } : {}
      ) as RequestHeaders,
      ...init
    })

    if (response.getStatusCode() >= 200 && response.getStatusCode() < 300) {
      const data = await response.toJSON()

      return data as Data
    }

    // Surface the server-provided error message alongside the status code; a bare
    // `Error("400")` hides the actual failure cause (validation issue, timeout, plan limit).
    let serverMessage = ''
    let errorBody: unknown
    try {
      const body = (await response.toJSON()) as Record<string, any>
      errorBody = body
      const message = body?.message ?? body?.error
      if (Array.isArray(message)) {
        serverMessage = message.join('; ')
      } else if (typeof message === 'string') {
        serverMessage = message
      }
    } catch {
      // Non-JSON or empty error body — fall back to the status code alone.
    }

    const error = new Error(
      serverMessage ? `${response.getStatusCode()} ${serverMessage}` : `${response.getStatusCode()}`
    ) as Error & { status: number; body?: unknown }
    error.status = response.getStatusCode()
    error.body = errorBody
    throw error
  }
