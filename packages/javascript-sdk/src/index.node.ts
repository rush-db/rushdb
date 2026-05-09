import { HttpClient, HttpClientResponse } from './network/HttpClient.js'
import { NodeHttpClient } from './network/NodeHttpClient.js'
import { initSDK, RushDB } from './sdk/index.js'
import {
  type ApiResponse,
  type CreateEmbeddingIndexParams,
  type EmbeddingIndex,
  type EmbeddingIndexStats,
  type SemanticSearchParams,
  type SemanticSearchResult,
  type UpsertEmbeddingVectorItem,
  type UpsertEmbeddingVectorsParams,
  type UpsertEmbeddingVectorsResult,
  RestAPI
} from './api/index.js'

initSDK(new NodeHttpClient())

export {
  RushDB,
  HttpClient,
  HttpClientResponse,
  RestAPI,
  type ApiResponse,
  type EmbeddingIndex,
  type CreateEmbeddingIndexParams,
  type EmbeddingIndexStats,
  type SemanticSearchParams,
  type SemanticSearchResult,
  type UpsertEmbeddingVectorItem,
  type UpsertEmbeddingVectorsParams,
  type UpsertEmbeddingVectorsResult
}
export * from './types/index.js'
export * from './sdk/index.js'

export default RushDB
