import type { DBRecord } from '../sdk/record.js'
import type { Schema, SearchQuery, Where } from '../types/index.js'

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

/** Parameters for vector search over an embedding index. */
export type VectorSearchParams = {
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
  /** Max candidates to fetch from the vector index in direct vector-index mode. */
  topK?: number
  /** Maximum number of results to return (default 20). */
  limit?: number
}

/**
 * A record returned by db.records.vectorSearch().
 * Identical to DBRecord but with __score guaranteed present — never optional.
 * __score is the cosine similarity between the query vector and this record's embedding (0–1,
 * higher = more similar). It is only injected by the semantic search path; regular
 * db.records.find() / db.records.search() results are plain DBRecord and never carry __score.
 */
export type VectorSearchResult<S extends Schema = Schema> = DBRecord<S> & {
  readonly __score: number
}

/** @deprecated Use VectorSearchParams. */
export type SemanticSearchParams = VectorSearchParams

/** @deprecated Use VectorSearchResult. */
export type SemanticSearchResult<S extends Schema = Schema> = VectorSearchResult<S>

export type SmartSearchOptions<S extends Schema = Schema> = {
  /**
   * Optional current dashboard/query-builder state. The server may use it as context
   * when converting the natural-language prompt into a SearchQuery.
   */
  currentQuery?: SearchQuery<S>
}

export type SmartSearchQueryResponse<S extends Schema = Schema> = {
  searchQuery: SearchQuery<S>
  warnings: string[]
}

export type RelationshipPatternStatus = 'suggested' | 'approved' | 'ignored' | 'error'
export type RelationshipPatternOrigin = 'llm' | 'manual'
export type RelationshipPatternDirection = 'in' | 'out'
export type RelationshipPatternMode = 'join_pattern' | 'retype_existing_relationship'

export type RelationshipPatternEndpoint = {
  label: string
  key?: string
  where?: Where
}

export type RelationshipPatternDto = {
  id: string
  status: RelationshipPatternStatus
  origin: RelationshipPatternOrigin
  source: RelationshipPatternEndpoint
  target: RelationshipPatternEndpoint
  direction: RelationshipPatternDirection
  type: string
  mode: RelationshipPatternMode
  confidence: number
  rationale?: string
  sampleMatchCount?: number
  lastAppliedAt?: string
  lastAnalyzedAt?: string
  lastError?: string
  createdAt: string
  updatedAt: string
}

export type RelationshipPattern = RelationshipPatternDto

export type RelationshipPatternListResponse = {
  patterns: RelationshipPatternDto[]
  relationships: Array<{
    label: string
    relationships: Array<{ label: string; type: string; direction: string }>
  }>
  analysis?: {
    status: string
    requestedAt?: string
    notBefore?: string
    lastRunAt?: string
    lastError?: string
  }
}

export type DeleteRelationshipPatternOptions = {
  deleteExisting?: boolean
}

// ── Async import runs ────────────────────────────────────────────────

export type ImportFileFormat = 'csv' | 'jsonl' | 'ndjson' | 'json' | 'parquet'

export type ImportFileRole = 'records' | 'links'

export interface ImportLinkEndpoint {
  column: string
  label: string
  keyProperty: string
  direction: 'source' | 'target'
}

export interface ImportLinkSpec {
  version: 1
  role: 'links'
  endpoints: [ImportLinkEndpoint, ImportLinkEndpoint]
  relationshipType: string
  propertyColumns?: Record<string, string>
}

export type ImportFileManifest =
  | {
      clientFileId: string
      fileName: string
      size: number
      format: ImportFileFormat
      role?: 'records'
      rootLabel: string
      parseOptions?: Record<string, unknown>
      importOptions?: Record<string, unknown>
    }
  | {
      clientFileId: string
      fileName: string
      size: number
      format: ImportFileFormat
      role: 'links'
      linkSpec: ImportLinkSpec
      rootLabel?: never
      parseOptions?: Record<string, unknown>
      importOptions?: Record<string, unknown>
    }

export interface CreateImportRunParams {
  name?: string
  failurePolicy?: 'continue' | 'stop_new_files'
  files: Array<ImportFileManifest>
}

export interface CreateImportRunResponse {
  runId: string
  files: Array<{ fileId: string; clientFileId: string; suggestedLabel: string | null }>
}

export interface ImportRunFile {
  id: string
  runId: string
  ordinal: number
  clientFileId: string
  fileName: string
  declaredSizeBytes: number
  format: ImportFileFormat
  role: ImportFileRole
  rootLabel: string | null
  linkSpec: ImportLinkSpec | null
  status: string
  stage: string
  parsedUnits: number
  committedUnits: number
  recordsCommitted: number
  relationshipsCommitted: number
  linksResolved: number
  linksUnresolved: number
  skippedUnits: number
  attemptCount: number
  lastErrorCode: string | null
  lastErrorMessage: string | null
  waitingOn?: Array<{ fileId: string; status: string }>
}

export interface ImportRunEvent {
  id: string
  runId: string
  fileId: string | null
  type: string
  code?: string | null
  message?: string | null
  createdAt: string
}

export interface ImportRun {
  id: string
  projectId: string
  name: string | null
  status:
    | 'draft'
    | 'uploading'
    | 'queued'
    | 'running'
    | 'blocked'
    | 'canceling'
    | 'finalizing'
    | 'completed'
    | 'completed_with_errors'
    | 'failed'
    | 'canceled'
  failurePolicy: 'continue' | 'stop_new_files'
  totalFiles: number
  totalBytes: number
  uploadedBytes: number
  parsedUnits: number
  recordsCommitted: number
  relationshipsCommitted: number
  skippedUnits: number
  failedFiles: number
  cancelRequestedAt: string | null
  startedAt: string | null
  finalizedAt: string | null
  retentionUntil: string | null
  createdAt: string
  updatedAt: string
}

export interface ImportRunDetail extends ImportRun {
  files: Array<ImportRunFile>
  events: Array<ImportRunEvent>
}

export interface InitiateImportUploadResponse {
  uploadId: string
  storageKey: string
}

export interface SignPartParams {
  runId: string
  fileId: string
  partNumber: number
  uploadId: string
}

export interface SignPartResponse {
  url: string
  method: 'PUT'
  headers: Record<string, string>
  expiresInSeconds: number
}

export interface CompleteUploadParams {
  runId: string
  fileId: string
  uploadId: string
  expectedSizeBytes?: number
}

export interface UploadProgress {
  bytesUploaded: number
  totalBytes: number
  partsCompleted: number
  totalParts: number
}
