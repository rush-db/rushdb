import { HttpClient, HttpClientResponse } from './network/HttpClient.js'
import { NodeHttpClient } from './network/NodeHttpClient.js'
import { initSDK, RushDB } from './sdk/index.js'
import {
  type ApiResponse,
  type CreateEmbeddingIndexParams,
  type DeleteRelationshipPatternOptions,
  type EmbeddingIndex,
  type EmbeddingIndexStats,
  type RelationshipPattern,
  type RelationshipPatternDirection,
  type RelationshipPatternDto,
  type RelationshipPatternEndpoint,
  type RelationshipPatternListResponse,
  type RelationshipPatternMode,
  type RelationshipPatternOrigin,
  type RelationshipPatternStatus,
  type SemanticSearchParams,
  type SemanticSearchResult,
  type UpsertEmbeddingVectorItem,
  type UpsertEmbeddingVectorsParams,
  type UpsertEmbeddingVectorsResult,
  type VectorSearchParams,
  type VectorSearchResult,
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
  type DeleteRelationshipPatternOptions,
  type EmbeddingIndexStats,
  type RelationshipPattern,
  type RelationshipPatternDirection,
  type RelationshipPatternDto,
  type RelationshipPatternEndpoint,
  type RelationshipPatternListResponse,
  type RelationshipPatternMode,
  type RelationshipPatternOrigin,
  type RelationshipPatternStatus,
  type SemanticSearchParams,
  type SemanticSearchResult,
  type UpsertEmbeddingVectorItem,
  type UpsertEmbeddingVectorsParams,
  type UpsertEmbeddingVectorsResult,
  type VectorSearchParams,
  type VectorSearchResult
}
export * from './types/index.js'
export * from './sdk/index.js'

export default RushDB
