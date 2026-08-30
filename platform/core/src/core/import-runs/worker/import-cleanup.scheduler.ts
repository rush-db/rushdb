import { Inject, Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Cron } from '@nestjs/schedule'

import { ImportRunsRepository } from '../persistence/import-runs.repository'
import { IMPORT_STORAGE_PORT, type ImportStoragePort } from '../storage/import-storage.port'

/**
 * Applies source-object retention identically for every storage backend:
 * terminal, finalized runs past their retention window have their stored
 * sources deleted (graph data is never touched). Failures are left for the
 * next sweep.
 */
@Injectable()
export class ImportCleanupScheduler {
  private running = false

  constructor(
    private readonly repository: ImportRunsRepository,
    @Inject(IMPORT_STORAGE_PORT) private readonly storage: ImportStoragePort,
    private readonly configService: ConfigService
  ) {}

  @Cron('*/10 * * * *')
  async sweep(): Promise<void> {
    if (this.running) {
      return
    }
    this.running = true
    try {
      const now = Date.now()
      const runs = await this.repository.findRunsPastRetention(new Date(now).toISOString())

      for (const run of runs) {
        const files = await this.repository.listFilesWithSource(run.id)
        for (const file of files) {
          if (!file.storageKey) {
            continue
          }
          await this.storage.delete(file.storageKey)
          const cleared = await this.repository.clearSourceForFile(file.id)
          if (cleared) {
            await this.repository.addEvent({
              runId: run.id,
              fileId: file.id,
              projectId: run.projectId,
              type: 'SOURCE_CLEANED',
              code: 'RETENTION_EXPIRED'
            })
          }
        }
      }

      if (runs.length > 0) {
        Logger.log(`[ImportCleanup] retained-source cleanup processed ${runs.length} run(s)`)
      }
    } catch (error) {
      Logger.error('[ImportCleanup] retention sweep failed', error)
    } finally {
      this.running = false
    }
  }
}
