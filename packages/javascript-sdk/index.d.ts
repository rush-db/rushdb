import type { InferType, DBRecord, UserProvidedConfig, Relation } from './src/sdk'

import { DBRecordsBatchDraft, DBRecordDraft, RestAPI, ApiResponse } from './src/api'
import { HttpClient, HttpClientResponse } from './src/network/HttpClient'
import {
  Model,
  DBRecordInstance,
  DBRecordsArrayInstance,
  Transaction,
  idToDate,
  idToTimestamp,
  EmptyTargetError,
  UniquenessError,
  NonUniqueResultError
} from './src/sdk'
import { Models, Schema } from './src/types'

declare module '@rushdb/javascript-sdk' {
  export namespace RushDB {}

  export class RushDB extends RestAPI {
    static RushDB: typeof RushDB

    constructor(token?: string, config?: UserProvidedConfig)
    public api: RestAPI

    public registerModel<Model extends Model = Model>(model: Model): Model //Model<T['schema']>
    public getModel(label: string): Model
    public getModels(): Map<string, Model>
    public getInstance(token: string, config?: UserProvidedConfig): RushDB
    public toInstance<Schema extends Schema = Schema>(record: DBRecord<Schema>): DBRecordInstance
  }

  export {
    DBRecordsBatchDraft,
    Relation,
    ApiResponse,
    InferType,
    Model,
    Models,
    DBRecord,
    DBRecordDraft,
    DBRecordInstance,
    DBRecordsArrayInstance,
    RestAPI,
    Schema,
    Transaction,
    HttpClient,
    HttpClientResponse,
    EmptyTargetError,
    NonUniqueResultError,
    UniquenessError,
    idToDate,
    idToTimestamp
  }

  export * from './src/types'

  export default RushDB
}
