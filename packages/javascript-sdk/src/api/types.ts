import type { DBRecord } from '../sdk/record.js'
import type { Schema } from '../types/index.js'

export type ApiResponse<T, E = Record<string, any>> = {
  data: T
  success: boolean
  total?: number
} & E

/** An embedding index policy stored in RushDB. */
export type EmbeddingIndex = {
  id: string
  projectId: string
  /** Neo4j label this index is scoped to (e.g. "Book"). */
  label: string
  propertyName: string
  modelKey: string
  sourceType: 'managed' | 'external'
  similarityFunction: 'cosine' | 'euclidean'
  dimensions: number
  vectorPropertyName: string
  enabled: boolean
  /** 'pending' | 'indexing' | 'awaiting_vectors' | 'ready' | 'error' */
  status: string
  createdAt: string
  updatedAt: string
}

/** Parameters for creating a new embedding index. */
export type CreateEmbeddingIndexParams = {
  /** Neo4j label to scope this index to (e.g. "Book", "Task"). */
  label: string
  propertyName: string
  sourceType?: 'managed' | 'external'
  /**
   * Shorthand for `sourceType: 'external'`.
   * When `true`, the index will be created with `sourceType: 'external'` regardless of the `sourceType` field.
   */
  external?: boolean
  similarityFunction?: 'cosine' | 'euclidean'
  dimensions?: number
}

/**
 * A single vector entry for inline vector upsert.
 * Provided alongside record data in create/upsert/set calls.
 */
export type VectorEntry = {
  /** Name of the property whose embedding index should be written to. */
  propertyName: string
  /** The embedding vector to store. Its length must match the index dimensions. */
  vector: number[]
  /**
   * Required when two indexes share the same `propertyName` and `dimensions` but differ in
   * `similarityFunction`. Omit when there is only one matching index.
   */
  similarityFunction?: 'cosine' | 'euclidean'
}

export type UpsertEmbeddingVectorItem = {
  recordId: string
  vector: number[]
}

export type UpsertEmbeddingVectorsParams = {
  items: UpsertEmbeddingVectorItem[]
}

export type UpsertEmbeddingVectorsResult = {
  updated: number
  requested: number
}

/** Neo4j-level stats for an embedding index. */
export type EmbeddingIndexStats = {
  totalRecords: number
  indexedRecords: number
}

/** Parameters for semantic (vector) search over an embedding index. */
export type SemanticSearchParams = {
  /** Name of the indexed property to search against. */
  propertyName: string
  /** Free-text query that will be embedded and compared against indexed vectors. */
  query?: string
  /** External vector query. Use instead of query text for external indexes. */
  queryVector?: number[]
  /**
   * One or more Neo4j labels to scope the search.
   * The first label is used to resolve which embedding index to use.
   * Required — always provide at least one label.
   */
  labels: string[]
  sourceType?: 'managed' | 'external'
  similarityFunction?: 'cosine' | 'euclidean'
  dimensions?: number
  /**
   * Optional filter applied before cosine scoring.
   * Candidates are narrowed via MATCH/WHERE and then ranked by similarity.
   */
  where?: Record<string, unknown>
  /** Number of results to skip for pagination (default 0). */
  skip?: number
  /** Maximum number of results to return (default 20). */
  limit?: number
}

/**
 * A record returned by db.ai.search().
 * Identical to DBRecord but with __score guaranteed present — never optional.
 * __score is the cosine similarity between the query vector and this record's embedding (0–1,
 * higher = more similar). It is only injected by the semantic search path; regular
 * db.records.find() / db.records.search() results are plain DBRecord and never carry __score.
 */
export type SemanticSearchResult<S extends Schema = Schema> = DBRecord<S> & {
  readonly __score: number
}
