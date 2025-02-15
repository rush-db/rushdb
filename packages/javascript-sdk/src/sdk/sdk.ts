import type { HttpClient } from '../network/HttpClient.js'
import type { Models, Schema } from '../types/index.js'
import type { Model } from './model.js'
import type { DBRecord } from './record.js'
import type { State, SDKConfig } from './types.js'

import { RestAPI } from '../api/api.js'
import { DEFAULT_TIMEOUT } from '../common/constants.js'
import { DBRecordInstance } from './record.js'
import { parseConfig, validateInteger } from './utils.js'

export const initSDK = (httpClient: HttpClient) => {
  class RushDB extends RestAPI implements RushDB {
    static instance: RushDB
    state: State

    public models: Map<string, Model>

    constructor(token?: string, config?: SDKConfig) {
      const props = parseConfig(config)
      super(token, { ...props, httpClient: props.httpClient ?? httpClient })

      this.state = {
        debug: false,
        timeout: validateInteger('timeout', props.timeout, DEFAULT_TIMEOUT),
        token
      }
      this.models = new Map()

      RushDB.instance = this
    }

    public static getInstance(): RushDB {
      return RushDB.instance
    }

    public registerModel(model: Model) {
      const label = model.getLabel()

      // Inject the API into the model
      model.init(this)

      this.models.set(label, model)
      return model
    }

    public getModels(): Map<string, Model> {
      return this.models
    }

    public getModel<Label extends keyof Models | string = keyof Models>(
      label: Label
    ): Label extends keyof Models ? Model<Models[Label]> : Model | undefined {
      return this.models.get(label) as Label extends keyof Models ? Models[Label] : Model
    }

    public toInstance<S extends Schema = Schema>(record: DBRecord<S>) {
      const result = new DBRecordInstance<S>(record)
      result.init(this)
      return result
    }
  }

  return RushDB
}
