import { Module } from '@nestjs/common'

import { EntityModule } from '@/core/entity/entity.module'
import { ProjectModule } from '@/dashboard/project/project.module'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'

import { ImportRunsController } from './api/import-runs.controller'
import { ImportRunsService } from './import-runs.service'
import { ImportRunsRepository } from './persistence/import-runs.repository'
import { importStorageProvider, ImportStorageFactory } from './storage/import-storage.factory'
import { ImportCleanupScheduler } from './worker/import-cleanup.scheduler'
import { ImportWorkerService } from './worker/import-worker.service'

/**
 * Asynchronous multi-file import runs.
 *
 * Storage backend selection: RUSHDB_IMPORT_STORAGE_BACKEND=s3 (default) or
 * local; both implement ImportStoragePort and share retention cleanup.
 */
@Module({
  imports: [EntityModule, ProjectModule, WorkspaceModule],
  providers: [
    ImportRunsRepository,
    ImportRunsService,
    ImportStorageFactory,
    importStorageProvider,
    ImportWorkerService,
    ImportCleanupScheduler
  ],
  controllers: [ImportRunsController],
  exports: [ImportRunsService]
})
export class ImportRunsModule {}
