import type { HttpClientInterface } from '../network/HttpClient.js'

type ApiConnectionConfig =
  | {
      host: string
      port: number
      protocol: string
    }
  | {
      url: string
    }

export type State = {
  debug: boolean
  timeout: number
  token?: string
} & Partial<ApiConnectionConfig>

type CommonUserProvidedConfig = {
  httpClient?: HttpClientInterface
  timeout?: number
} & ApiConnectionConfig

export type UserProvidedConfig = CommonUserProvidedConfig
