import { Injectable } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'

import { IMPORT_STORAGE_PORT, type ImportStoragePort } from './import-storage.port'
import { LocalImportStorageAdapter } from './local-import-storage.adapter'
import { S3ImportStorageAdapter } from './s3-import-storage.adapter'

import type { ImportStorageBackend } from '../domain/import-run.types'

/**
 * Selects the import source backend:
 * - RUSHDB_IMPORT_STORAGE_BACKEND=s3 (default): S3-compatible multipart storage.
 * - RUSHDB_IMPORT_STORAGE_BACKEND=local: server-local filesystem storage.
 *
 * Both backends share the ImportStoragePort contract; retention/cleanup runs
 * identically against either through port.delete().
 */
@Injectable()
export class ImportStorageFactory {
  constructor(private readonly configService: ConfigService) {}

  get backend(): ImportStorageBackend {
    const raw = this.configService.get<string>('RUSHDB_IMPORT_STORAGE_BACKEND', 's3').toLowerCase()
    if (raw !== 's3' && raw !== 'local') {
      throw new Error(`RUSHDB_IMPORT_STORAGE_BACKEND must be "s3" or "local", got: ${raw}`)
    }
    return raw
  }

  create(): ImportStoragePort {
    return this.backend === 'local' ? this.createLocal() : this.createS3()
  }

  private createLocal(): ImportStoragePort {
    const rootPath = this.configService.get<string>('RUSHDB_IMPORT_STORAGE_LOCAL_PATH')
    return new LocalImportStorageAdapter(rootPath)
  }

  private createS3(): ImportStoragePort {
    return new S3ImportStorageAdapter({
      bucket: this.configService.get<string>('RUSHDB_IMPORT_S3_BUCKET'),
      region: this.configService.get<string>('RUSHDB_IMPORT_S3_REGION'),
      endpoint: this.configService.get<string>('RUSHDB_IMPORT_S3_ENDPOINT'),
      accessKeyId: this.configService.get<string>('RUSHDB_IMPORT_S3_ACCESS_KEY_ID'),
      secretAccessKey: this.configService.get<string>('RUSHDB_IMPORT_S3_SECRET_ACCESS_KEY'),
      forcePathStyle: this.configService.get<string>('RUSHDB_IMPORT_S3_FORCE_PATH_STYLE') === 'true'
    })
  }
}

export const importStorageProvider = {
  provide: IMPORT_STORAGE_PORT,
  useFactory: (factory: ImportStorageFactory) => factory.create(),
  inject: [ImportStorageFactory]
}
