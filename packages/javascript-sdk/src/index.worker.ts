import { FetchHttpClient } from './network/FetchHttpClient.js'
import { HttpClient, HttpClientResponse } from './network/HttpClient.js'
import { initSDK } from './sdk/index.js'
import { type ApiResponse, RestAPI } from './api/index.js'

const RushDB = initSDK(new FetchHttpClient())

export { RushDB, HttpClient, HttpClientResponse, RestAPI, type ApiResponse }
export * from './types/index.js'
export * from './sdk/index.js'

export default RushDB
