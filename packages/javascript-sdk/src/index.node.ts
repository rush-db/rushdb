import { HttpClient, HttpClientResponse } from './network/HttpClient.js'
import { NodeHttpClient } from './network/NodeHttpClient.js'
import {
  DBRecordsBatchDraft,
  Model,
  type DBRecord,
  DBRecordInstance,
  DBRecordDraft,
  DBRecordsArrayInstance,
  Transaction,
  EmptyTargetError,
  NonUniqueResultError,
  UniquenessError,
  initSDK,
  idToDate,
  idToTimestamp,
  type Relation,
  type InferType
} from './sdk/index.js'
import { type ApiResponse, RestAPI } from './api/index.js'

const RushDB = initSDK(new NodeHttpClient())

export {
  RushDB,
  DBRecordsBatchDraft,
  DBRecordDraft,
  Model,
  DBRecord,
  type Relation,
  DBRecordInstance,
  DBRecordsArrayInstance,
  Transaction,
  EmptyTargetError,
  HttpClient,
  HttpClientResponse,
  NonUniqueResultError,
  UniquenessError,
  RestAPI,
  type InferType,
  type ApiResponse,
  idToDate,
  idToTimestamp
}
export * from './types/index.js'

export default RushDB
