import type { HttpClient } from '../network/HttpClient.js'
import type { Schema } from '../types/index.js'

import type { DBRecord } from './record.js'
import { DBRecordInstance } from './record.js'
import type { SDKConfig, State } from './types.js'

import { RestAPI } from '../api/api.js'
import { DEFAULT_TIMEOUT } from '../common/constants.js'
import { parseConfig, validateInteger } from './utils.js'
import { toBoolean } from '../common/utils.js'

let httpClient: HttpClient

export class RushDB extends RestAPI {
  static instance: RushDB | null = null

  public static state: State = {
    initialized: false,
    debug: false,
    timeout: DEFAULT_TIMEOUT
  }

  constructor(token?: string, config?: SDKConfig) {
    const resolvedToken = token ?? (typeof process !== 'undefined' ? process.env?.RUSHDB_API_KEY : undefined)
    const props = parseConfig(config)
    super(resolvedToken, { ...props, httpClient: props.httpClient ?? httpClient })

    RushDB.instance = this
    this.initializeSync(resolvedToken, props)
  }

  private initializeSync(token?: string, props?: any) {
    RushDB.state = {
      initialized: true,
      debug: false,
      timeout: validateInteger('timeout', props?.timeout, DEFAULT_TIMEOUT),
      token
    }
  }

  /**
   * Synchronous getInstance that returns instance or null
   */
  public static getInstance(): RushDB {
    if (RushDB.instance) {
      return RushDB.instance
    }

    throw new Error(
      'RushDB not initialized. Please create a RushDB instance first: new RushDB("RUSHDB_API_KEY")'
    )
  }

  /**
   * Check if the SDK is initialized
   */
  public static isInitialized(): boolean {
    return toBoolean(RushDB.instance)
  }

  public toDBRecordInstance<S extends Schema = Schema>(record: DBRecord<S>) {
    return new DBRecordInstance<S>(record)
  }
}

export const initSDK = (client: HttpClient) => {
  httpClient = client
}
