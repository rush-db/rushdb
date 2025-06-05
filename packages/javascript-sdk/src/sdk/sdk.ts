import type { HttpClient } from '../network/HttpClient.js'
import type { Schema } from '../types/index.js'

import type { DBRecord } from './record.js'
import { DBRecordInstance } from './record.js'
import type { SDKConfig, State } from './types.js'

import { RestAPI } from '../api/api.js'
import { DEFAULT_TIMEOUT } from '../common/constants.js'
import { parseConfig, validateInteger } from './utils.js'

let httpClient: HttpClient

export class RushDB extends RestAPI {
  static instance: RushDB | null = null
  static initPromise: Promise<RushDB> | null = null

  public static state: State = {
    initialized: false,
    debug: false,
    timeout: DEFAULT_TIMEOUT
  }

  constructor(token?: string, config?: SDKConfig) {
    const props = parseConfig(config)
    super(token, { ...props, httpClient: props.httpClient ?? httpClient })

    RushDB.instance = this

    RushDB.initPromise = this.initializeAsync(token, props).catch((error) => {
      console.error('RushDB initialization failed:', error)
      throw error
    })
  }

  /**
   * Returns a promise that resolves when the SDK is fully initialized
   * Use this to ensure the SDK is ready before using it
   */
  public async waitForInitialization(): Promise<RushDB> {
    if (RushDB.initPromise) {
      return await RushDB.initPromise
    }
    return this
  }

  private async initializeAsync(token?: string, props?: any): Promise<RushDB> {
    try {
      const { data: serverSettings } = await this.settings.get()

      RushDB.state = {
        initialized: true,
        debug: false,
        timeout: validateInteger('timeout', props?.timeout, DEFAULT_TIMEOUT),
        token,
        serverSettings
      }

      return this
    } catch (error) {
      console.error('Failed to initialize RushDB:', error)
      throw new Error(`RushDB initialization failed: ${error}`)
    } finally {
      RushDB.state.initialized = true
    }
  }

  /**
   * Static method to get or wait for initialization
   * This is the preferred way to get an initialized RushDB instance
   */
  static async init(): Promise<RushDB> {
    // If already initialized, return the instance
    if (RushDB.state.initialized && RushDB.instance) {
      return RushDB.instance
    }

    // If initialization is in progress, wait for it
    if (RushDB.initPromise) {
      return await RushDB.initPromise
    }

    // If an instance exists but is not yet initialized, wait for it
    if (RushDB.instance) {
      return await RushDB.instance.waitForInitialization()
    }

    throw new Error(
      'RushDB not initialized. Please create a RushDB instance first: new RushDB("RUSHDB_API_TOKEN")'
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
    return RushDB.state.initialized && RushDB.instance !== null
  }

  public toDBRecordInstance<S extends Schema = Schema>(record: DBRecord<S>) {
    const result = new DBRecordInstance<S>(record)
    return result
  }
}

export const initSDK = (client: HttpClient) => {
  httpClient = client
}
