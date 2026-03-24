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
  dimensions: number
  enabled: boolean
  /** 'pending' | 'indexing' | 'ready' | 'error' */
  status: string
  createdAt: string
  updatedAt: string
}

/** Parameters for creating a new embedding index. */
export type CreateEmbeddingIndexParams = {
  /** Neo4j label to scope this index to (e.g. "Book", "Task"). */
  label: string
  propertyName: string
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
  query: string
  /**
   * One or more Neo4j labels to scope the search.
   * The first label is used to resolve which embedding index to use.
   * Required — always provide at least one label.
   */
  labels: string[]
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
