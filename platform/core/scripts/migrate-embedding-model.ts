import { NestFactory } from '@nestjs/core'
import { SchedulerRegistry } from '@nestjs/schedule'

import { EmbeddingModelMigrationService } from '@/core/ai/embedding-model-migration.service'
import { AppModule } from '@/app.module'

/**
 * Manual entry point for the embedding-model migration (see
 * EmbeddingModelMigrationService for what it does and why).
 *
 * The same migration also runs automatically once per server boot, so a normal prod
 * deploy that changes RUSHDB_EMBEDDING_MODEL needs no manual step. Use this script to
 * preview the impact before deploying or to re-drive a failed startup run.
 *
 * Usage (from platform/core, against the same env the server runs with):
 *   pnpm embeddings:migrate-model            # apply
 *   pnpm embeddings:migrate-model --dry-run  # report what would change, touch nothing
 */
async function migrateEmbeddingModel() {
  const dryRun = process.argv.includes('--dry-run')

  // The script drives the migration explicitly below — keep the automatic on-boot run
  // out of this process, otherwise a dry run would still apply changes.
  EmbeddingModelMigrationService.suppressStartupRun = true

  const app = await NestFactory.createApplicationContext(AppModule, {
    logger: ['error', 'warn', 'log']
  })

  try {
    // The app context registers the platform's cron jobs (embedding backfill, etc.).
    // This process only re-keys rows — stop the crons so re-embedding stays with the
    // long-running server instead of racing inside a short-lived script.
    const schedulerRegistry = app.get(SchedulerRegistry)
    for (const [name, job] of schedulerRegistry.getCronJobs()) {
      job.stop()
      console.log(`[migrate-embedding-model] stopped cron "${name}" for this process`)
    }

    const migrationService = app.get(EmbeddingModelMigrationService)
    const summary = await migrationService.migrateStaleIndexes({ dryRun })
    console.log(
      `[migrate-embedding-model] target="${summary.targetModel}" ${dryRun ? 'would migrate' : 'migrated'}=${summary.migrated} skipped=${summary.skipped}`
    )
  } finally {
    await app.close()
  }
}

void migrateEmbeddingModel()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error('[migrate-embedding-model] failed:', error)
    process.exit(1)
  })
