import { HttpClient, HttpClientResponse } from './network/HttpClient.js'
import { NodeHttpClient } from './network/NodeHttpClient.js'
import { initSDK } from './sdk/index.js'
import { type ApiResponse, RestAPI } from './api/index.js'

const RushDB = initSDK(new NodeHttpClient())

export { RushDB, HttpClient, HttpClientResponse, RestAPI, type ApiResponse }
export * from './types/index.js'
export * from './sdk/index.js'

export default RushDB
