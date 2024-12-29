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

module.exports = {
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
  default: RushDB,
  idToDate,
  idToTimestamp
}

module.exports.RushDB = RushDB

module.exports.default = RushDB
