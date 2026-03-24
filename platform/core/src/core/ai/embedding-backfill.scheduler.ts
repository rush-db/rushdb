import { Injectable, Logger } from '@nestjs/common'
import { Cron } from '@nestjs/schedule'
import { ConfigService } from '@nestjs/config'

import { AiQueryService } from '@/core/ai/ai-query.service'
import { EmbeddingIndexRepository } from '@/core/ai/embedding-index.repository'
import { EmbeddingProviderService } from '@/core/ai/embedding-provider.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
export class EmbeddingBackfillScheduler {
  private readonly logger = new Logger(EmbeddingBackfillScheduler.name)
  private running = false

  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly aiQueryService: AiQueryService,
    private readonly embeddingIndexRepository: EmbeddingIndexRepository,
    private readonly embeddingProviderService: EmbeddingProviderService,
    private readonly configService: ConfigService
  ) {}

  @Cron('* * * * *') // every minute
  async runBackfill(): Promise<void> {
    if (this.running) return
    this.running = true

    try {
      await this.backfillPending()
    } catch (err) {
      this.logger.error('[EmbeddingBackfill] unexpected error', err)
    } finally {
      this.running = false
    }
  }

  private async backfillPending(): Promise<void> {
    const pending = await this.embeddingIndexRepository.findPending()
    if (pending.length === 0) return

    const batchSize = Number(this.configService.get('RUSHDB_EMBEDDING_BATCH_SIZE') ?? 500)
    const maxRuntimeMs = Number(this.configService.get('RUSHDB_EMBEDDING_MAX_RUNTIME_MS') ?? 50_000)

    await Promise.allSettled(pending.map((index) => this.backfillIndex(index, batchSize, maxRuntimeMs)))
  }

  private async backfillIndex(
    index: { id: string; projectId: string; propertyName: string; label: string },
    batchSize: number,
    maxRuntimeMs: number
  ): Promise<void> {
    await this.embeddingIndexRepository.updateStatus(index.id, 'indexing')

    // propKey is written to rel.__propKey and used by the ANN post-filter for label isolation.
    // Format: "Label:propertyName" (e.g. "Book:title").
    const propKey = `${index.label}:${index.propertyName}`
    const labelSuffix = `:${index.label}`

    const deadline = Date.now() + maxRuntimeMs
    let skip = 0

    try {
      while (Date.now() < deadline) {
        const session = this.neogmaService.createSession('embedding-backfill')
        let relations: Array<{ relId: string; value: unknown }>

        try {
          const result = await session.run(this.aiQueryService.getUnindexedRelationsQuery(labelSuffix), {
            projectId: index.projectId,
            propertyName: index.propertyName,
            propKey,
            skip,
            batchSize
          })

          relations = result.records.map((r) => ({
            relId: r.get('relId') as string,
            value: r.get('value')
          }))
        } finally {
          await session.close()
        }

        if (relations.length === 0) break

        // Build texts to embed (array values join with space; other types coerce to string)
        const texts = relations.map(({ value }) =>
          Array.isArray(value) ? (value as string[]).join(' ') : String(value)
        )

        const embeddings = await this.embeddingProviderService.embedBatch(texts)

        const updates = relations
          .map((rel, i) => ({
            relId: rel.relId,
            emb: embeddings[i],
            projectId: index.projectId,
            propKey: index.propertyName
          }))
          .filter((u) => u.emb != null)

        if (updates.length > 0) {
          const writeSession = this.neogmaService.createSession('embedding-backfill-write')
          try {
            const tx = writeSession.beginTransaction({ timeout: 30_000 })
            try {
              await tx.run(this.aiQueryService.getWriteEmbeddingsQuery(), { updates })
              await tx.commit()
            } catch (err) {
              await tx.rollback()
              throw err
            }
          } finally {
            await writeSession.close()
          }
        }

        if (relations.length < batchSize) break
        skip += batchSize
      }

      // Check if any unindexed remain (timed out case)
      const checkSession = this.neogmaService.createSession('embedding-backfill-check')
      let remaining = 0
      try {
        const result = await checkSession.run(this.aiQueryService.getUnindexedCountQuery(labelSuffix), {
          projectId: index.projectId,
          propertyName: index.propertyName,
          propKey
        })
        const raw = result.records[0]?.get('remaining')
        remaining = typeof raw?.toNumber === 'function' ? raw.toNumber() : Number(raw ?? 0)
      } finally {
        await checkSession.close()
      }

      await this.embeddingIndexRepository.updateStatus(index.id, remaining === 0 ? 'ready' : 'pending')
    } catch (err) {
      this.logger.error(`[EmbeddingBackfill] index ${index.id} failed: ${err}`)
      await this.embeddingIndexRepository.updateStatus(index.id, 'error')
    }
  }
}
