// Local mirror of the SDK's async-import types (packages/javascript-sdk/src/api/types.ts).
// The SDK does not re-export these from its public entry point, so we keep a copy here.
// Keep in sync with the SDK + backend contract.

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
