import { Injectable, Logger, OnApplicationBootstrap } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'

import { AiQueryService } from '@/core/ai/ai-query.service'
import { EmbeddingIndexRepository } from '@/core/ai/embedding-index.repository'
import { EmbeddingProviderService } from '@/core/ai/embedding-provider.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

export interface EmbeddingModelMigrationSummary {
  targetModel: string
  migrated: number
  skipped: number
}

/**
 * Re-keys managed embedding indexes when RUSHDB_EMBEDDING_MODEL changes.
 *
 * Vectors from different models are incompatible even at identical dimensions — they
 * live in different embedding spaces — yet they share the same relationship property
 * slot (vectorPropertyName encodes sourceType/similarity/dimensions, NOT the model).
 * Leaving old vectors in place after a model switch silently corrupts similarity scores.
 *
 * For every enabled managed index whose stored modelKey differs from the configured
 * model: strip the old vectors (exact __propKey match, so co-named properties of other
 * labels/projects are untouched), then re-key the row to the configured model with
 * status 'pending'. The backfill scheduler re-embeds on its next cycles — and it only
 * picks up indexes keyed to its own configured model, so instances still running the
 * old env during a rolling deploy cannot write stale vectors into re-keyed indexes.
 *
 * Runs automatically once per boot. The run is idempotent: when every managed index
 * already matches the configured model it exits after one cheap SQL check. Concurrent
 * runs from parallel booting instances are safe — stripping is idempotent and all
 * instances converge on the same modelKey.
 *
 * Rows whose dimensions differ from RUSHDB_EMBEDDING_DIMENSIONS are skipped with a
 * warning: a dimension change moves vectors to a different slot and Neo4j index, which
 * is a delete-and-recreate operation, not a re-key.
 */
@Injectable()
export class EmbeddingModelMigrationService implements OnApplicationBootstrap {
  /**
   * Set by scripts/migrate-embedding-model.ts before it boots the app context: the
   * script drives the migration explicitly (with --dry-run support), so the automatic
   * on-boot run inside that process must stay off — otherwise a dry run would still
   * apply changes.
   */
  static suppressStartupRun = false

  private readonly logger = new Logger(EmbeddingModelMigrationService.name)

  constructor(
    private readonly configService: ConfigService,
    private readonly embeddingIndexRepository: EmbeddingIndexRepository,
    private readonly embeddingProviderService: EmbeddingProviderService,
    private readonly neogmaService: NeogmaService,
    private readonly aiQueryService: AiQueryService
  ) {}

  onApplicationBootstrap(): void {
    if (EmbeddingModelMigrationService.suppressStartupRun) {
      return
    }

    const { targetModel, targetDimensions } = this.getTargetConfig()
    if (!targetModel || !targetDimensions) {
      // Embeddings not configured on this deployment — nothing to migrate against.
      return
    }

    // Fire-and-forget: a failed or slow migration (provider outage, large strip) must
    // never block or crash the boot; the next deploy/restart retries it.
    void this.migrateStaleIndexes().catch((err) => {
      this.logger.error(
        `startup embedding-model migration failed (retries on next boot, or run \`pnpm embeddings:migrate-model\` manually): ${err instanceof Error ? err.message : err}`
      )
    })
  }

  private getTargetConfig(): { targetModel: string; targetDimensions: number } {
    const targetModel = this.configService.get<string>('RUSHDB_EMBEDDING_MODEL') ?? ''
    const dimensionsRaw = this.configService.get<string>('RUSHDB_EMBEDDING_DIMENSIONS')
    const parsed = Number.parseInt(dimensionsRaw ?? '0', 10)
    const targetDimensions = Number.isInteger(parsed) && parsed > 0 ? parsed : 0
    return { targetModel, targetDimensions }
  }

  async migrateStaleIndexes({
    dryRun = false
  }: { dryRun?: boolean } = {}): Promise<EmbeddingModelMigrationSummary> {
    const { targetModel, targetDimensions } = this.getTargetConfig()
    if (!targetModel || !targetDimensions) {
      throw new Error('RUSHDB_EMBEDDING_MODEL and RUSHDB_EMBEDDING_DIMENSIONS (positive integer) must be set')
    }

    const staleRows = await this.embeddingIndexRepository.findManagedWithStaleModelKey(targetModel)
    if (staleRows.length === 0) {
      this.logger.log(`all managed indexes already use "${targetModel}" — nothing to migrate`)
      return { targetModel, migrated: 0, skipped: 0 }
    }

    // Fail before touching any data if the configured provider/model cannot actually
    // serve vectors of the expected size.
    const probe = await this.embeddingProviderService.embed('rushdb-embedding-model-migration-probe')
    if (probe.length !== targetDimensions) {
      throw new Error(
        `Provider returned ${probe.length}-dimensional vectors for "${targetModel}", expected ${targetDimensions}`
      )
    }

    this.logger.log(
      `${staleRows.length} managed index(es) to migrate to "${targetModel}"${dryRun ? ' (dry run)' : ''}`
    )

    let migrated = 0
    let skipped = 0

    for (const row of staleRows) {
      const indexRef = `${row.id} project=${row.projectId} ${row.label}:${row.propertyName}`

      if (row.dimensions !== targetDimensions) {
        skipped += 1
        this.logger.warn(
          `SKIP ${indexRef} — row has ${row.dimensions} dims but the target model is configured for ${targetDimensions}. ` +
            `A dimension change needs the index deleted and recreated (different vector slot).`
        )
        continue
      }

      this.logger.log(
        `${dryRun ? 'would migrate' : 'migrating'} ${indexRef} "${row.modelKey}" → "${targetModel}"`
      )

      if (dryRun) {
        migrated += 1
        continue
      }

      const labelSuffix = row.label ? `:${row.label}` : ''
      const propKey = `${row.label}:${row.propertyName}`
      const session = this.neogmaService.createSession('embedding-model-migration')
      try {
        await session.run(this.aiQueryService.getStripEmbeddingsQuery(labelSuffix, row.vectorPropertyName), {
          projectId: row.projectId,
          propertyName: row.propertyName,
          propKey
        })
      } finally {
        await session.close()
      }

      await this.embeddingIndexRepository.updateModelKeyAndStatus(row.id, targetModel, 'pending')
      migrated += 1
    }

    this.logger.log(
      `embedding-model migration done: ${migrated} ${dryRun ? 'would be migrated' : 'migrated'}, ${skipped} skipped` +
        (dryRun || migrated === 0 ?
          ''
        : ' — the backfill scheduler will re-embed the stripped properties with the new model on its next cycles')
    )

    return { targetModel, migrated, skipped }
  }
}
