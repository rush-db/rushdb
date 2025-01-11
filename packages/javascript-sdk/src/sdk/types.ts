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

export type Logger = (payload: any) => void

type CommonUserProvidedConfig = {
  httpClient?: HttpClientInterface
  timeout?: number
  logger?: Logger
} & ApiConnectionConfig

export type UserProvidedConfig = CommonUserProvidedConfig
