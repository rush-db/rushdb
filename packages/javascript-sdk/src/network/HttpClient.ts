import type { RequestData, RequestHeaders, ResponseHeaders } from './types.js'

type TimeoutError = TypeError & { code?: string }

export interface HttpClientInterface {
  makeRequest: (url: string, config: MakeRequestConfig) => Promise<HttpClientResponseInterface>
}

export interface MakeRequestConfig<T extends Record<string, any> = Record<string, any>> {
  credentials?: RequestCredentials
  headers?: RequestHeaders
  method: string
  protocol?: string
  requestData?: RequestData
  timeout?: number
}

export interface HttpClientResponseInterface {
  getHeaders: () => ResponseHeaders
  getRawResponse: () => unknown
  getStatusCode: () => number
  toJSON: () => Promise<any>
  toStream: (streamCompleteCallback: () => void) => unknown
}

export class HttpClient implements HttpClientInterface {
  static CONNECTION_CLOSED_ERROR_CODES: Array<string>
  static TIMEOUT_ERROR_CODE: string

  makeRequest(
    url: string,
    { headers, method, protocol, requestData, timeout }: MakeRequestConfig
  ): Promise<HttpClientResponseInterface> {
    throw new Error('makeRequest not implemented.')
  }

  static makeTimeoutError(): TimeoutError {
    const timeoutErr: TimeoutError = new TypeError(HttpClient.TIMEOUT_ERROR_CODE)
    timeoutErr.code = HttpClient.TIMEOUT_ERROR_CODE
    return timeoutErr
  }
}

HttpClient.CONNECTION_CLOSED_ERROR_CODES = ['ECONNRESET', 'EPIPE']
HttpClient.TIMEOUT_ERROR_CODE = 'ETIMEDOUT'

export class HttpClientGenericResponse implements HttpClientResponseInterface {
  _statusCode: number
  _headers: ResponseHeaders

  constructor(statusCode: number, headers: ResponseHeaders) {
    this._statusCode = statusCode
    this._headers = headers
  }

  getStatusCode(): number {
    return this._statusCode
  }

  getHeaders(): ResponseHeaders {
    return this._headers
  }

  getRawResponse(): unknown {
    throw new Error('getRawResponse not implemented.')
  }

  toStream(streamCompleteCallback: () => void): unknown {
    throw new Error('toStream not implemented.')
  }

  toJSON(): any {
    throw new Error('toJSON not implemented.')
  }
}

export class HttpClientResponse<T = unknown> implements HttpClientResponseInterface {
  _res: T

  constructor(response: T) {
    this._res = response
  }

  getStatusCode(): number {
    throw new Error('getStatusCode not implemented.')
  }

  getHeaders(): ResponseHeaders {
    throw new Error('getHeaders not implemented.')
  }

  getRawResponse(): unknown {
    throw new Error('getRawResponse not implemented.')
  }

  toStream(streamCompleteCallback: () => void): unknown {
    throw new Error('toStream not implemented.')
  }

  toJSON(): any {
    throw new Error('toJSON not implemented.')
  }
}
