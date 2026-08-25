import {
  AbortMultipartUploadCommand,
  CompleteMultipartUploadCommand,
  CreateMultipartUploadCommand,
  DeleteObjectCommand,
  GetObjectCommand,
  HeadObjectCommand,
  PutObjectCommand,
  S3Client,
  UploadPartCommand
} from '@aws-sdk/client-s3'
import { getSignedUrl } from '@aws-sdk/s3-request-presigner'
import { Injectable, Logger } from '@nestjs/common'

import type {
  ImportStoragePort,
  InitiateUploadResult,
  SignedPartRequest,
  StorageObjectMeta
} from './import-storage.port'

const MULTIPART_CHUNK_THRESHOLD = 8 * 1024 * 1024

export interface S3ImportStorageConfig {
  bucket: string
  region?: string
  endpoint?: string
  accessKeyId?: string
  secretAccessKey?: string
  forcePathStyle?: boolean
}

interface PendingUpload {
  storageKey: string
  uploadId: string
  partNumber: number
  parts: Array<{ ETag: string; PartNumber: number }>
}

/**
 * S3-compatible multipart backend for import sources (default:
 * RUSHDB_IMPORT_STORAGE_BACKEND=s3). Works with AWS S3 and any S3-compatible
 * endpoint (MinIO, R2, etc.). Retention/cleanup uses the same delete() contract
 * as the local backend.
 */
@Injectable()
export class S3ImportStorageAdapter implements ImportStoragePort {
  private readonly client: S3Client
  private readonly bucket: string
  private readonly pending = new Map<string, PendingUpload>()
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private readonly logger = new Logger(S3ImportStorageAdapter.name)

  constructor(config?: Partial<S3ImportStorageConfig>) {
    this.bucket = config?.bucket ?? process.env.RUSHDB_IMPORT_S3_BUCKET ?? ''

    if (!this.bucket) {
      throw new Error('S3 import storage requires RUSHDB_IMPORT_S3_BUCKET')
    }

    this.client = new S3Client({
      region: config?.region ?? process.env.RUSHDB_IMPORT_S3_REGION ?? undefined,
      endpoint: config?.endpoint ?? process.env.RUSHDB_IMPORT_S3_ENDPOINT ?? undefined,
      forcePathStyle:
        config?.forcePathStyle ??
        (process.env.RUSHDB_IMPORT_S3_FORCE_PATH_STYLE ?
          process.env.RUSHDB_IMPORT_S3_FORCE_PATH_STYLE === 'true'
        : false),
      credentials:
        config?.accessKeyId && config?.secretAccessKey ?
          { accessKeyId: config.accessKeyId, secretAccessKey: config.secretAccessKey }
        : process.env.RUSHDB_IMPORT_S3_ACCESS_KEY_ID && process.env.RUSHDB_IMPORT_S3_SECRET_ACCESS_KEY ?
          {
            accessKeyId: process.env.RUSHDB_IMPORT_S3_ACCESS_KEY_ID,
            secretAccessKey: process.env.RUSHDB_IMPORT_S3_SECRET_ACCESS_KEY
          }
        : undefined
    })
  }

  async initiate(projectId: string, fileId: string, sourceGeneration: number): Promise<InitiateUploadResult> {
    const storageKey = `imports/${projectId}/${fileId}/g${sourceGeneration}/${Date.now()}-${Math.random()
      .toString(36)
      .slice(2, 10)}`

    const result = await this.client.send(
      new CreateMultipartUploadCommand({ Bucket: this.bucket, Key: storageKey })
    )

    if (!result.UploadId) {
      throw new Error(`failed to initiate multipart upload for ${storageKey}`)
    }

    this.pending.set(storageKey, {
      storageKey,
      uploadId: result.UploadId as string,
      partNumber: 0,
      parts: []
    })

    return { uploadId: result.UploadId as string, storageKey, directUpload: true }
  }

  /**
   * Presigns a single part PUT so the browser can upload directly to S3 without
   * bytes traversing the API. Requires the multipart session initiated above.
   */
  async signPart(storageKey: string, partNumber: number): Promise<SignedPartRequest | null> {
    const pendingUpload = this.pending.get(storageKey)
    if (!pendingUpload) {
      return null
    }

    const command = new UploadPartCommand({
      Bucket: this.bucket,
      Key: storageKey,
      UploadId: pendingUpload.uploadId,
      PartNumber: partNumber
    })

    const url = await getSignedUrl(this.client, command, { expiresIn: 900 })

    return { url, method: 'PUT', headers: {}, expiresInSeconds: 900 }
  }

  async writeChunk(storageKey: string, data: Buffer): Promise<void> {
    const pendingUpload = this.ensurePending(storageKey)

    if (pendingUpload.parts.length === 0 && data.byteLength < MULTIPART_CHUNK_THRESHOLD) {
      // Small sources go through simple PUT; complete() short-circuits to PutObject semantics.
      await this.client.send(new PutObjectCommand({ Bucket: this.bucket, Key: storageKey, Body: data }))
      pendingUpload.partNumber = -1
      return
    }

    pendingUpload.partNumber += 1
    const part = await this.client.send(
      new UploadPartCommand({
        Bucket: this.bucket,
        Key: storageKey,
        UploadId: pendingUpload.uploadId,
        PartNumber: pendingUpload.partNumber,
        Body: data
      })
    )

    if (!part.ETag) {
      throw new Error(`part ${pendingUpload.partNumber} of ${storageKey} returned no ETag`)
    }
    pendingUpload.parts.push({ ETag: part.ETag, PartNumber: pendingUpload.partNumber })
  }

  async complete(storageKey: string, expectedSizeBytes?: number): Promise<StorageObjectMeta> {
    let meta = await this.head(storageKey)

    if (!meta) {
      const pendingUpload = this.pending.get(storageKey)
      if (!pendingUpload || pendingUpload.partNumber === -1) {
        throw new Error(`object not found after completion: ${storageKey}`)
      }

      await this.client.send(
        new CompleteMultipartUploadCommand({
          Bucket: this.bucket,
          Key: storageKey,
          UploadId: pendingUpload.uploadId,
          MultipartUpload: { Parts: pendingUpload.parts }
        })
      )
      this.pending.delete(storageKey)
      meta = await this.head(storageKey)
    } else {
      // Object already stored via simple PUT; release the still-open multipart
      // upload so no orphaned parts linger.
      const pendingUpload = this.pending.get(storageKey)
      this.pending.delete(storageKey)
      if (pendingUpload?.uploadId) {
        await this.client
          .send(
            new AbortMultipartUploadCommand({
              Bucket: this.bucket,
              Key: storageKey,
              UploadId: pendingUpload.uploadId
            })
          )
          .catch(() => undefined)
      }
    }

    if (!meta) {
      throw new Error(`object not found after completion: ${storageKey}`)
    }
    if (expectedSizeBytes !== undefined && expectedSizeBytes !== meta.sizeBytes) {
      throw new Error(`size mismatch for ${storageKey}: expected ${expectedSizeBytes}, got ${meta.sizeBytes}`)
    }
    return meta
  }

  async head(storageKey: string): Promise<StorageObjectMeta | null> {
    try {
      const result = await this.client.send(new HeadObjectCommand({ Bucket: this.bucket, Key: storageKey }))
      return { storageKey, sizeBytes: result.ContentLength ?? 0 }
    } catch {
      return null
    }
  }

  async readStream(storageKey: string): Promise<NodeJS.ReadableStream> {
    const result = await this.client.send(new GetObjectCommand({ Bucket: this.bucket, Key: storageKey }))
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    if (!result.Body) {
      throw new Error(`empty body for object ${storageKey}`)
    }
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    return result.Body as unknown as NodeJS.ReadableStream
  }

  async abort(storageKey: string): Promise<void> {
    try {
      // Only in-process uploads can be aborted by ID; orphaned multipart uploads
      // from crashed workers are covered by the bucket lifecycle rule.
      const uploadId = this.pending.get(storageKey)?.uploadId

      if (uploadId) {
        await this.client.send(
          new AbortMultipartUploadCommand({ Bucket: this.bucket, Key: storageKey, UploadId: uploadId })
        )
      }
    } catch (error) {
      this.logger.warn(`failed to abort multipart upload for ${storageKey}`, error as Error)
    } finally {
      this.pending.delete(storageKey)
    }
  }

  async delete(storageKey: string): Promise<void> {
    try {
      await this.abort(storageKey).catch(() => undefined)
      await this.client.send(new DeleteObjectCommand({ Bucket: this.bucket, Key: storageKey }))
    } catch (error) {
      this.logger.warn(`failed to delete object ${storageKey}`, error as Error)
    }
  }

  private ensurePending(storageKey: string): PendingUpload {
    const pendingUpload = this.pending.get(storageKey)
    if (!pendingUpload) {
      throw new Error(`no initiated upload for key: ${storageKey} (call initiate first)`)
    }
    return pendingUpload
  }
}
