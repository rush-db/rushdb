import { Injectable } from '@nestjs/common'

import { PassThrough } from 'node:stream'

import type { ImportStoragePort, InitiateUploadResult, StorageObjectMeta } from './import-storage.port'

/**
 * Process-local storage adapter for local vertical slice and tests.
 * Objects do not survive process restarts; production deployments bind an
 * S3-compatible adapter to the same port.
 */
@Injectable()
export class MemoryImportStorageAdapter implements ImportStoragePort {
  private readonly objects = new Map<string, { chunks: Buffer[]; size: number; completed: boolean }>()
  private counter = 0

  async initiate(projectId: string, fileId: string, sourceGeneration: number): Promise<InitiateUploadResult> {
    const uploadId = `mem-${++this.counter}`
    const storageKey = `imports/${projectId}/${fileId}/g${sourceGeneration}/${uploadId}`
    this.objects.set(storageKey, { chunks: [], size: 0, completed: false })
    return { uploadId, storageKey, directUpload: false }
  }

  async writeChunk(storageKey: string, data: Buffer): Promise<void> {
    const object = this.objects.get(storageKey)
    if (!object) {
      throw new Error(`unknown storage key: ${storageKey}`)
    }
    if (object.completed) {
      throw new Error(`object already completed: ${storageKey}`)
    }
    object.chunks.push(data)
    object.size += data.byteLength
  }

  async complete(storageKey: string, expectedSizeBytes?: number): Promise<StorageObjectMeta> {
    const object = this.objects.get(storageKey)
    if (!object) {
      throw new Error(`unknown storage key: ${storageKey}`)
    }
    if (expectedSizeBytes !== undefined && expectedSizeBytes !== object.size) {
      throw new Error(`size mismatch for ${storageKey}: expected ${expectedSizeBytes}, got ${object.size}`)
    }
    object.completed = true
    return { storageKey, sizeBytes: object.size }
  }

  async head(storageKey: string): Promise<StorageObjectMeta | null> {
    const object = this.objects.get(storageKey)
    if (!object || !object.completed) {
      return null
    }
    return { storageKey, sizeBytes: object.size }
  }

  async readStream(storageKey: string): Promise<NodeJS.ReadableStream> {
    const object = this.objects.get(storageKey)
    if (!object || !object.completed) {
      throw new Error(`object not available: ${storageKey}`)
    }
    const stream = new PassThrough()
    for (const chunk of object.chunks) {
      stream.write(chunk)
    }
    stream.end()
    return stream
  }

  async abort(storageKey: string): Promise<void> {
    this.objects.delete(storageKey)
  }

  async delete(storageKey: string): Promise<void> {
    this.objects.delete(storageKey)
  }
}
