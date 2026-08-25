import { Injectable } from '@nestjs/common'

import { createReadStream, createWriteStream, existsSync, mkdirSync, statSync } from 'node:fs'
import { mkdir, rm } from 'node:fs/promises'
import { dirname, join, normalize, resolve, sep } from 'node:path'

import type { ImportStoragePort, InitiateUploadResult, StorageObjectMeta } from './import-storage.port'

/**
 * Filesystem storage backend for import sources (RUSHDB_IMPORT_STORAGE_BACKEND=local).
 * Objects live under a server-owned root directory; retention/cleanup uses the
 * same delete() contract as the S3 backend.
 */
@Injectable()
export class LocalImportStorageAdapter implements ImportStoragePort {
  private readonly root: string

  constructor(rootPath?: string) {
    this.root = resolve(rootPath ?? process.env.RUSHDB_IMPORT_STORAGE_LOCAL_PATH ?? '.rushdb-imports')
    if (!existsSync(this.root)) {
      mkdirSync(this.root, { recursive: true })
    }
  }

  async initiate(projectId: string, fileId: string, sourceGeneration: number): Promise<InitiateUploadResult> {
    const uploadId = `local-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
    return {
      uploadId,
      storageKey: `imports/${projectId}/${fileId}/g${sourceGeneration}/${uploadId}`,
      directUpload: false
    }
  }

  async writeChunk(storageKey: string, data: Buffer): Promise<void> {
    const path = this.toPath(storageKey)
    await mkdir(dirname(path), { recursive: true })
    await new Promise<void>((resolvePromise, reject) => {
      const stream = createWriteStream(path, { flags: 'a' })
      stream.on('error', reject)
      stream.on('finish', () => resolvePromise())
      stream.end(data)
    })
  }

  async complete(storageKey: string, expectedSizeBytes?: number): Promise<StorageObjectMeta> {
    const path = this.toPath(storageKey)
    if (!existsSync(path)) {
      throw new Error(`object not found: ${storageKey}`)
    }
    const size = statSync(path).size
    if (expectedSizeBytes !== undefined && expectedSizeBytes !== size) {
      throw new Error(`size mismatch for ${storageKey}: expected ${expectedSizeBytes}, got ${size}`)
    }
    return { storageKey, sizeBytes: size }
  }

  async head(storageKey: string): Promise<StorageObjectMeta | null> {
    try {
      const path = this.toPath(storageKey)
      if (!existsSync(path)) {
        return null
      }
      return { storageKey, sizeBytes: statSync(path).size }
    } catch {
      return null
    }
  }

  readStream(storageKey: string): Promise<NodeJS.ReadableStream> {
    const path = this.toPath(storageKey)
    if (!existsSync(path)) {
      return Promise.reject(new Error(`object not found: ${storageKey}`))
    }
    return Promise.resolve(createReadStream(path))
  }

  async abort(storageKey: string): Promise<void> {
    await this.removeQuietly(storageKey)
  }

  async delete(storageKey: string): Promise<void> {
    await this.removeQuietly(storageKey)
  }

  /** Retention sweep helper shared by the cleanup scheduler. */
  async deletePrefix(prefix: string): Promise<number> {
    const target = join(this.root, normalize(prefix))
    if (!target.startsWith(this.root + sep) && target !== this.root) {
      throw new Error('invalid prefix')
    }
    if (!existsSync(target)) {
      return 0
    }
    await rm(target, { recursive: true, force: true })
    return 1
  }

  private async removeQuietly(storageKey: string): Promise<void> {
    try {
      await rm(this.toPath(storageKey), { force: true })
    } catch {
      /* deletion failures are retried by the retention sweeper */
    }
  }

  private toPath(storageKey: string): string {
    if (storageKey.includes('..')) {
      throw new Error(`invalid storage key: ${storageKey}`)
    }
    const normalized = normalize(storageKey)
    const path = join(this.root, normalized)
    if (!path.startsWith(this.root + sep) && path !== this.root) {
      throw new Error(`invalid storage key: ${storageKey}`)
    }
    return path
  }
}
