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
        typeof token !== 'undefined' ? { token } : {}
      ) as RequestHeaders,
      ...init
    })

    if (response.getStatusCode() >= 200 && response.getStatusCode() < 300) {
      const data = await response.toJSON()

      return data as Data
    }

    // @TODO: Make it more useful
    throw new Error(`${response.getStatusCode()}`)
  }
