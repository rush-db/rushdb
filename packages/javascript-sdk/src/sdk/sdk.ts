import type { HttpClient } from '../network/HttpClient.js'
import type { Schema } from '../types/index.js'

import type { DBRecord } from './record.js'
import { DBRecordInstance } from './record.js'
import { SDKConfig, State, TokenPublicVariables } from './types.js'

import { RestAPI } from '../api/api.js'
import { DEFAULT_TIMEOUT } from '../common/constants.js'
import { extractMixedPropertiesFromToken, parseConfig, validateInteger } from './utils.js'

let httpClient: HttpClient

export class RushDB extends RestAPI {
  static instance: RushDB | null = null

  public static state: State = {
    initialized: false,
    debug: false,
    timeout: DEFAULT_TIMEOUT
  }

  constructor(token?: string, config?: SDKConfig) {
    const props = parseConfig(config)
    super(token, { ...props, httpClient: props.httpClient ?? httpClient })

    RushDB.instance = this
    const [maybeMixed] = extractMixedPropertiesFromToken(token ?? '')
    this.initializeSync(maybeMixed, token, props)
  }

  private initializeSync(mixed: TokenPublicVariables | null, token?: string, props?: any) {
    const serverSettings =
      mixed ?
        {
          planType: mixed.planType,
          customDB: mixed.customDB,
          managedDB: mixed.managedDB,
          selfHosted: mixed.selfHosted
        }
      : null

    RushDB.state = {
      initialized: true,
      debug: false,
      timeout: validateInteger('timeout', props?.timeout, DEFAULT_TIMEOUT),
      token,
      ...(serverSettings && { serverSettings })
    }
  }

  /**
   * Static method to get or wait for initialization
   * This is the preferred way to get an initialized RushDB instance
   */
  static async init(): Promise<RushDB> {
    // If an instance exists but is not yet initialized, wait for it
    if (RushDB.instance) {
      return RushDB.instance
    }

    throw new Error(
      'RushDB not initialized. Please create a RushDB instance first: new RushDB("RUSHDB_API_KEY")'
    )
  }

  /**
   * Synchronous getInstance that returns instance or null
   */
  public static getInstance(): RushDB | null {
    return RushDB.instance
  }

  /**
   * Check if the SDK is initialized
   */
  public static isInitialized(): boolean {
    return RushDB.instance !== null
  }

  public toDBRecordInstance<S extends Schema = Schema>(record: DBRecord<S>) {
    return new DBRecordInstance<S>(record)
  }
}

export const initSDK = (client: HttpClient) => {
  httpClient = client
}
