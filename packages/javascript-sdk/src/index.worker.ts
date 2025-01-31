import { FetchHttpClient } from './network/FetchHttpClient.js'
import { HttpClient, HttpClientResponse } from './network/HttpClient.js'
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
  type InferType
} from './sdk/index.js'
import { type ApiResponse, RestAPI } from './api/index.js'

const RushDB = initSDK(new FetchHttpClient())

export {
  RushDB,
  DBRecordsBatchDraft,
  DBRecordDraft,
  Model,
  type DBRecord,
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
