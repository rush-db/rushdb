export interface InitiateUploadResult {
  uploadId: string
  storageKey: string
  /** Present when the backend supports browser-direct multipart (S3). */
  directUpload: boolean
}

export interface StorageObjectMeta {
  storageKey: string
  sizeBytes: number
}

export interface SignedPartRequest {
  url: string
  method: 'PUT'
  headers: Record<string, string>
  expiresInSeconds: number
}

/**
 * Port over import source bytes. Two adapters implement it:
 * - S3-compatible multipart storage (default; supports presigned browser-direct uploads)
 * - Local filesystem storage
 * Retention/cleanup runs identically against either through delete().
 */
export interface ImportStoragePort {
  initiate(projectId: string, fileId: string, sourceGeneration: number): Promise<InitiateUploadResult>
  writeChunk(storageKey: string, data: Buffer): Promise<void>
  complete(storageKey: string, expectedSizeBytes?: number): Promise<StorageObjectMeta>
  head(storageKey: string): Promise<StorageObjectMeta | null>
  readStream(storageKey: string): Promise<NodeJS.ReadableStream>
  abort(storageKey: string): Promise<void>
  delete(storageKey: string): Promise<void>

  /**
   * Signs a single multipart part for browser-direct upload. Backends without
   * presigning capability return null and callers fall back to proxied upload.
   */
  signPart?(storageKey: string, partNumber: number): Promise<SignedPartRequest | null>
}

export const IMPORT_STORAGE_PORT = Symbol('IMPORT_STORAGE_PORT')
