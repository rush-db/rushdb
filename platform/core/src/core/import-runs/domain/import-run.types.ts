export const IMPORT_RUN_STATUSES = [
  'draft',
  'uploading',
  'queued',
  'running',
  'blocked',
  'canceling',
  'finalizing',
  'completed',
  'completed_with_errors',
  'failed',
  'canceled'
] as const

export type ImportRunStatus = (typeof IMPORT_RUN_STATUSES)[number]

export const IMPORT_FILE_STATUSES = [
  'awaiting_upload',
  'uploading',
  'uploaded',
  'queued',
  'validating',
  'running',
  'retry_wait',
  'blocked',
  'finalizing',
  'completed',
  'failed',
  'canceled',
  'source_expired'
] as const

export type ImportFileStatus = (typeof IMPORT_FILE_STATUSES)[number]

export const TERMINAL_FILE_STATUSES: ImportFileStatus[] = [
  'completed',
  'failed',
  'canceled',
  'source_expired'
]

export const RECORDS_PHASE_TERMINAL_STATUSES: ImportFileStatus[] = ['completed', 'failed', 'canceled']

export type ImportFileStage = 'upload' | 'validate' | 'parse' | 'write' | 'finalize'

export type ImportFailurePolicy = 'continue' | 'stop_new_files'

export type ImportFileFormat = 'csv' | 'jsonl' | 'ndjson' | 'json' | 'parquet'

export type ImportStorageBackend = 's3' | 'local'

export type ImportFileRole = 'records' | 'links'

export type ImportRunOutcome = Extract<
  ImportRunStatus,
  'completed' | 'completed_with_errors' | 'failed' | 'canceled'
>

export const IMPORT_ERROR_CODES = {
  MANIFEST_CONFLICT: 'IMPORT_MANIFEST_CONFLICT',
  FILE_LIMIT_EXCEEDED: 'IMPORT_FILE_LIMIT_EXCEEDED',
  RUN_BYTES_EXCEEDED: 'IMPORT_RUN_BYTES_EXCEEDED',
  FORMAT_UNSUPPORTED: 'IMPORT_FORMAT_UNSUPPORTED',
  LABEL_INVALID: 'IMPORT_LABEL_INVALID',
  UPLOAD_EXPIRED: 'IMPORT_UPLOAD_EXPIRED',
  CHECKSUM_MISMATCH: 'IMPORT_CHECKSUM_MISMATCH',
  SOURCE_MISSING: 'IMPORT_SOURCE_MISSING',
  PARSE_ERROR: 'IMPORT_PARSE_ERROR',
  TOO_MANY_ERRORS: 'IMPORT_TOO_MANY_ERRORS',
  LINK_SPEC_INVALID: 'IMPORT_LINK_SPEC_INVALID',
  LINK_COLUMNS_UNMAPPED: 'IMPORT_LINK_COLUMNS_UNMAPPED',
  LINK_ENDPOINT_UNRESOLVED: 'IMPORT_LINK_ENDPOINT_UNRESOLVED',
  LINK_LABEL_EMPTY: 'IMPORT_LINK_LABEL_EMPTY',
  REPLAY_ID_COLLISION: 'IMPORT_REPLAY_ID_COLLISION',
  QUOTA_BLOCKED: 'IMPORT_QUOTA_BLOCKED',
  CANCELED: 'IMPORT_CANCELED',
  LEASE_LOST: 'IMPORT_LEASE_LOST',
  FINALIZATION_DELAYED: 'IMPORT_FINALIZATION_DELAYED',
  INTERNAL: 'IMPORT_INTERNAL_ERROR'
} as const

export type ImportErrorCode = keyof typeof IMPORT_ERROR_CODES

export type ImportLinkEndpointDirection = 'source' | 'target'

export interface ImportLinkEndpoint {
  column: string
  label: string
  keyProperty: string
  direction: ImportLinkEndpointDirection
}

export interface ImportLinkSpec {
  version: 1
  role: 'links'
  endpoints: [ImportLinkEndpoint, ImportLinkEndpoint]
  relationshipType: string
  propertyColumns?: Record<string, string>
}

export interface ImportFileManifestItem {
  clientFileId: string
  fileName: string
  size: number
  format: ImportFileFormat
  role: ImportFileRole
  rootLabel?: string
  linkSpec?: ImportLinkSpec
  parseOptions?: Record<string, unknown>
  importOptions?: Record<string, unknown>
}

export interface ImportCheckpoint {
  version: 1
  sourceGeneration: number
  nextUnit: number
  lastCommittedBatch: number
}

export interface ImportCounters {
  parsedUnits: number
  committedUnits: number
  recordsCommitted: number
  relationshipsCommitted: number
  linksResolved: number
  linksUnresolved: number
  skippedUnits: number
}
