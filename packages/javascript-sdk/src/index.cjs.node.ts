import { HttpClient, HttpClientResponse } from './network/HttpClient.js'
import { NodeHttpClient } from './network/NodeHttpClient.js'
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

const RushDB = initSDK(new NodeHttpClient())

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
