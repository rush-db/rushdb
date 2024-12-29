import { FetchHttpClient } from './network/FetchHttpClient.js'
import { HttpClient, HttpClientResponse } from './network/HttpClient.js'
import {
  DBRecordsBatchDraft,
  Model,
  DBRecordInstance,
  DBRecordsArrayInstance,
  Transaction,
  EmptyTargetError,
  NonUniqueResultError,
  UniquenessError,
  initSDK,
  idToDate,
  idToTimestamp
} from './sdk/index.js'

const RushDB = initSDK(new FetchHttpClient())

export {
  RushDB,
  DBRecordsBatchDraft,
  Model,
  DBRecordInstance,
  DBRecordsArrayInstance,
  Transaction,
  EmptyTargetError,
  HttpClient,
  HttpClientResponse,
  NonUniqueResultError,
  UniquenessError,
  idToDate,
  idToTimestamp
}
export default RushDB
