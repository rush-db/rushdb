import { Inject, Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'

import {
  IMPORT_ERROR_CODES,
  type ImportFileManifestItem,
  type ImportLinkSpec
} from './domain/import-run.types'
import { detectFormatFromFileName, suggestLabelFromFileName } from './domain/label-suggestion'
import { ImportValidationError, validateLinkSpec } from './domain/link-spec.validation'
import { hashIdempotencyKey } from './domain/replay-identity'
import { ImportRunsRepository } from './persistence/import-runs.repository'
import { IMPORT_STORAGE_PORT, type ImportStoragePort } from './storage/import-storage.port'

export interface CreateImportRunInput {
  projectId: string
  workspaceId?: string
  name?: string
  failurePolicy?: 'continue' | 'stop_new_files'
  files: ImportFileManifestItem[]
}

@Injectable()
export class ImportRunsService {
  constructor(
    private readonly repository: ImportRunsRepository,
    @Inject(IMPORT_STORAGE_PORT) private readonly storage: ImportStoragePort,
    private readonly configService: ConfigService
  ) {}

  get enabled(): boolean {
    return this.configService.get<string>('RUSHDB_IMPORTS_ENABLED', 'true') !== 'false'
  }

  async createDraft(
    input: CreateImportRunInput,
    idempotencyKeyHash?: string
  ): Promise<{
    runId: string
    files: Array<{ fileId: string; clientFileId: string; suggestedLabel: string | null }>
  }> {
    if (!this.enabled) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.FORMAT_UNSUPPORTED, 'async imports are disabled')
    }

    if (!Array.isArray(input.files) || input.files.length === 0) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.FILE_LIMIT_EXCEEDED, 'at least one file is required')
    }

    for (const item of input.files) {
      this.validateManifestItem(item)
    }

    if (idempotencyKeyHash) {
      const existing = await this.repository.getRunByIdempotencyKey(input.projectId, idempotencyKeyHash)
      if (existing && existing.status === 'draft') {
        const files = await this.repository.listFiles(existing.id)
        return {
          runId: existing.id,
          files: files.map((f) => ({
            fileId: f.id,
            clientFileId: f.clientFileId,
            suggestedLabel: f.rootLabel
          }))
        }
      }
    }

    const run = await this.repository.createRun({
      projectId: input.projectId,
      workspaceId: input.workspaceId ?? null,
      name: input.name ?? null,
      status: 'draft',
      failurePolicy: input.failurePolicy ?? 'continue',
      manifestVersion: 0,
      idempotencyKeyHash: idempotencyKeyHash ?? null,
      totalFiles: input.files.length,
      totalBytes: input.files.reduce((acc, f) => acc + (f.size || 0), 0),
      createdByType: 'user'
    })

    const created = await this.repository.createFiles(
      input.files.map((item, ordinal) => ({
        runId: run.id,
        projectId: input.projectId,
        workspaceId: input.workspaceId ?? null,
        ordinal,
        clientFileId: item.clientFileId,
        fileName: item.fileName.slice(0, 255),
        declaredSizeBytes: Math.max(0, Math.floor(item.size || 0)),
        format: item.format,
        role: item.role,
        rootLabel: item.role === 'records' ? item.rootLabel?.toUpperCase() : null,
        linkSpec: item.role === 'links' ? JSON.stringify(item.linkSpec) : null,
        parseOptions: item.parseOptions ? JSON.stringify(item.parseOptions) : null,
        importOptions: item.importOptions ? JSON.stringify(item.importOptions) : null,
        status: 'awaiting_upload',
        stage: 'upload'
      }))
    )

    return {
      runId: run.id,
      files: created.map((f) => ({
        fileId: f.id,
        clientFileId: f.clientFileId,
        suggestedLabel: f.rootLabel
      }))
    }
  }

  /** Uploads source bytes into the storage port and marks the file queued. */
  async uploadFileContent(
    projectId: string,
    runId: string,
    fileId: string,
    content: Buffer
  ): Promise<{ status: string; storageKey: string }> {
    const file = await this.getDraftOrUploadableFile(runId, fileId, projectId)

    if (file.objectSizeBytes && file.storageKey) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.UPLOAD_EXPIRED, 'file already uploaded')
    }

    const declared = file.declaredSizeBytes ?? 0
    if (declared > 0 && content.byteLength !== declared) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.CHECKSUM_MISMATCH, 'size mismatch')
    }

    const initiated = await this.storage.initiate(projectId, fileId, file.sourceGeneration)
    await this.storage.writeChunk(initiated.storageKey, content)
    const meta = await this.storage.complete(initiated.storageKey, content.byteLength)

    await this.repository.updateFile(fileId, {
      storageProvider: 'memory',
      storageKey: meta.storageKey,
      objectSizeBytes: meta.sizeBytes,
      status: 'queued',
      stage: 'validate'
    })

    await this.repository.updateRun(runId, { status: 'uploading' })
    await this.repository.addEvent({
      runId,
      fileId,
      projectId,
      type: 'FILE_UPLOADED',
      toStatus: 'queued',
      metadata: JSON.stringify({ sizeBytes: meta.sizeBytes })
    })

    return { status: 'queued', storageKey: meta.storageKey }
  }

  /**
   * Prepares a browser-direct multipart upload session. Returns whether the
   * active backend supports presigned parts; clients fall back to proxied
   * content upload when it does not.
   */
  async initiateUpload(projectId: string, runId: string, fileId: string) {
    const file = await this.getDraftOrUploadableFile(runId, fileId, projectId)

    if (file.objectSizeBytes && file.storageKey) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.UPLOAD_EXPIRED, 'file already uploaded')
    }

    const initiated = await this.storage.initiate(projectId, fileId, file.sourceGeneration)
    await this.repository.updateFile(fileId, {
      storageProvider: 's3',
      storageKey: initiated.storageKey,
      storageUploadId: initiated.uploadId,
      status: 'uploading'
    })

    return {
      uploadId: initiated.uploadId,
      storageKey: initiated.storageKey,
      directUpload: initiated.directUpload
    }
  }

  /** Signs one multipart part for browser-direct PUT. Null when unsupported. */
  async signPart(projectId: string, runId: string, fileId: string, uploadId: string, partNumber: number) {
    const file = await this.getDraftOrUploadableFile(runId, fileId, projectId)

    if (!file.storageKey || file.storageUploadId !== uploadId) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.UPLOAD_EXPIRED, 'unknown or stale upload session')
    }

    if (!this.storage.signPart) {
      return null
    }

    return this.storage.signPart(file.storageKey, Math.max(1, Math.min(10_000, Math.floor(partNumber))))
  }

  /** Completes a browser-direct multipart upload and queues the file. */
  async completeUpload(
    projectId: string,
    runId: string,
    fileId: string,
    uploadId: string,
    expectedSizeBytes?: number
  ) {
    const file = await this.getDraftOrUploadableFile(runId, fileId, projectId)

    if (!file.storageKey || file.storageUploadId !== uploadId) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.UPLOAD_EXPIRED, 'unknown or stale upload session')
    }

    const meta = await this.storage.complete(file.storageKey, expectedSizeBytes)

    await this.repository.updateFile(fileId, {
      objectSizeBytes: meta.sizeBytes,
      status: 'queued',
      stage: 'validate'
    })
    await this.repository.updateRun(runId, { status: 'uploading' })
    await this.repository.addEvent({
      runId,
      fileId,
      projectId,
      type: 'FILE_UPLOADED',
      toStatus: 'queued',
      metadata: JSON.stringify({ sizeBytes: meta.sizeBytes, mode: 'direct' })
    })

    return { status: 'queued' }
  }

  /** Aborts an in-flight browser-direct upload session. */
  async abortUpload(projectId: string, runId: string, fileId: string, uploadId: string) {
    const file = await this.getDraftOrUploadableFile(runId, fileId, projectId)

    if (file.storageKey && file.storageUploadId === uploadId && !file.objectSizeBytes) {
      await this.storage.abort(file.storageKey)
      await this.repository.updateFile(fileId, {
        storageKey: null,
        storageUploadId: null,
        status: 'awaiting_upload'
      })
    }
  }

  async startRun(runId: string, projectId: string): Promise<void> {
    const run = await this.repository.getRun(runId, projectId)
    if (!run) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.SOURCE_MISSING, 'run not found')
    }
    if (run.status !== 'draft') {
      throw new ImportValidationError(
        IMPORT_ERROR_CODES.MANIFEST_CONFLICT,
        `cannot start run in status ${run.status}`
      )
    }

    const files = await this.repository.listFiles(runId)
    const uploaded = files.filter((f) => f.objectSizeBytes !== null && f.objectSizeBytes !== undefined)
    if (uploaded.length !== files.length) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.SOURCE_MISSING, 'not all files are uploaded')
    }

    await this.repository.updateRun(runId, {
      status: 'queued',
      startedAt: new Date().toISOString(),
      manifestVersion: run.manifestVersion + 1
    })

    await this.repository.addEvent({
      runId,
      fileId: null,
      projectId,
      type: 'RUN_STARTED',
      toStatus: 'queued'
    })
  }

  async cancelRun(runId: string, projectId: string): Promise<void> {
    const run = await this.repository.getRun(runId, projectId)
    if (!run) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.SOURCE_MISSING, 'run not found')
    }
    await this.repository.requestCancel(runId, projectId)
    await this.maybeFinalizeCanceledRun(runId)
  }

  async retryRun(runId: string, projectId: string): Promise<void> {
    const run = await this.repository.getRun(runId, projectId)
    if (!run) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.SOURCE_MISSING, 'run not found')
    }
    const files = await this.repository.listFiles(runId)
    const retryable = files.filter((f) => f.status === 'failed')

    for (const file of retryable) {
      await this.repository.updateFile(file.id, {
        status: 'queued',
        notBefore: null,
        lastErrorCode: null,
        lastErrorMessage: null
      })
    }

    if (retryable.length > 0) {
      await this.repository.updateRun(runId, { status: 'queued', finalizedAt: null })
    }
  }

  async getRunDetail(runId: string, projectId: string) {
    const run = await this.repository.getRun(runId, projectId)
    if (!run) {
      return null
    }
    const [files, events] = await Promise.all([
      this.repository.listFiles(runId),
      this.repository.listEvents(runId, projectId)
    ])
    return { ...run, files, events }
  }

  async listRuns(projectId: string) {
    return this.repository.listRuns(projectId)
  }

  async deleteDraftRun(runId: string, projectId: string): Promise<void> {
    const run = await this.repository.getRun(runId, projectId)
    if (!run) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.SOURCE_MISSING, 'run not found')
    }
    if (run.status !== 'draft') {
      throw new ImportValidationError(IMPORT_ERROR_CODES.MANIFEST_CONFLICT, 'only draft runs can be deleted')
    }
    const files = await this.repository.listFiles(runId)
    for (const file of files) {
      if (file.storageKey) {
        await this.storage.delete(file.storageKey).catch(() => undefined)
      }
    }
    await this.repository.deleteDraftRun(runId, projectId)
  }

  hashKey(key: string): string {
    return hashIdempotencyKey(key)
  }

  // ------------------------------------------------------------------

  private async getDraftOrUploadableFile(runId: string, fileId: string, projectId: string) {
    const run = await this.repository.getRun(runId, projectId)
    if (!run) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.SOURCE_MISSING, 'run not found')
    }
    const file = await this.repository.getFile(fileId, projectId)
    if (!file || file.runId !== runId) {
      throw new ImportValidationError(IMPORT_ERROR_CODES.SOURCE_MISSING, 'file not found')
    }
    return file
  }

  private validateManifestItem(item: ImportFileManifestItem): void {
    if (!item.clientFileId || !item.fileName) {
      throw new ImportValidationError(
        IMPORT_ERROR_CODES.MANIFEST_CONFLICT,
        'clientFileId and fileName are required'
      )
    }

    const supported = ['csv', 'jsonl', 'ndjson', 'json', 'parquet']
    if (!item.format || !supported.includes(item.format)) {
      throw new ImportValidationError(
        IMPORT_ERROR_CODES.FORMAT_UNSUPPORTED,
        `unsupported format for ${item.fileName}; supported: ${supported.join(', ')}`
      )
    }

    if (item.role === 'links') {
      try {
        validateLinkSpec(item.linkSpec)
      } catch (error) {
        if (error instanceof ImportValidationError) {
          throw error
        }
        throw new ImportValidationError(IMPORT_ERROR_CODES.LINK_SPEC_INVALID, (error as Error).message)
      }
      if (item.rootLabel) {
        throw new ImportValidationError(
          IMPORT_ERROR_CODES.LABEL_INVALID,
          'links files must not carry a rootLabel'
        )
      }
    } else {
      if (!item.rootLabel) {
        item.rootLabel = suggestLabelFromFileName(item.fileName)
      }
      if (!/^[A-Za-z][A-Za-z0-9_]{0,99}$/.test(item.rootLabel)) {
        throw new ImportValidationError(
          IMPORT_ERROR_CODES.LABEL_INVALID,
          `invalid root label: ${item.rootLabel}`
        )
      }
    }
  }

  private async maybeFinalizeCanceledRun(runId: string): Promise<void> {
    const files = await this.repository.listFiles(runId)
    const unfinished = files.filter((f) => ['validating', 'running'].includes(f.status))
    if (unfinished.length > 0) {
      return
    }
    await this.repository.refreshRunAggregates(runId)
    const canceledAll = files.every((f) => f.status === 'canceled')
    await this.repository.updateRun(runId, {
      status: canceledAll ? 'canceled' : 'completed_with_errors',
      finalizedAt: new Date().toISOString()
    })
  }
}
