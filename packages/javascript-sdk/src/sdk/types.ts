import type { HttpClientInterface } from '../network/HttpClient.js'
import { PlanPrefix } from './constants'

type ApiConnectionConfig =
  | {
      host?: string
      port?: number
      protocol?: string
    }
  | {
      url?: string
    }

export type State = {
  debug: boolean
  timeout: number
  token?: string
  initialized: boolean
  serverSettings?: {
    selfHosted: boolean
    dashboardUrl?: string
    googleOAuthEnabled?: boolean
    githubOAuthEnabled?: boolean
    customDB?: boolean
    managedDB?: boolean
    canceled?: boolean
    planType?: PlanType
  }
} & Partial<ApiConnectionConfig>

export type Logger = (payload: any) => void

export type SDKConfig = {
  httpClient?: HttpClientInterface
  timeout?: number
  logger?: Logger
  options?: {
    /**
     * @description
     * Defaults to `false`.
     * Allows using the `delete()` method without a specified criteria,
     * which results in deleting all Records in the project.
     */
    allowForceDelete?: boolean
  }
} & ApiConnectionConfig

export type PlanType = keyof typeof PlanPrefix

type RawServerSettings = NonNullable<State['serverSettings']>
export type TokenPublicVariables = Pick<
  RawServerSettings,
  'selfHosted' | 'customDB' | 'managedDB' | 'canceled' | 'planType'
>
