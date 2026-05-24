import { HttpException, Inject, Injectable, Logger } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Cron } from '@nestjs/schedule'
import { int as neo4jInt } from 'neo4j-driver'

import { isDevMode } from '@/common/utils/isDevMode'
import { AiQueryService } from '@/core/ai/ai-query.service'
import { EmbeddingIndexRepository } from '@/core/ai/embedding-index.repository'
import { EmbeddingProviderService } from '@/core/ai/embedding-provider.service'
import { estimateBatchTokens, estimateEmbeddingBatchKu } from '@/core/ai/embedding.utils'
import { BILLING_POLICY_PORT, BillingPolicyPort } from '@/core/billing-policy/billing-policy.port'
import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'
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
    private readonly configService: ConfigService,
    @Inject(BILLING_POLICY_PORT)
    private readonly billingPolicyService: BillingPolicyPort,
    private readonly kuEventsService: KuEventsService,
    private readonly projectRepository: ProjectRepository
  ) {}

  @Cron('* * * * *') // every minute
  async runBackfill(): Promise<void> {
    if (this.running) {
      return
    }
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

    isDevMode(() => this.logger.debug(`[backfillPending] pending indexes: ${pending.length}`))

    if (pending.length === 0) {
      return
    }

    const batchSizeRaw = this.configService.get<string>('RUSHDB_EMBEDDING_BATCH_SIZE')
    const batchSizeParsed = Number.parseInt(batchSizeRaw ?? '500', 10)
    const batchSize = Number.isInteger(batchSizeParsed) && batchSizeParsed > 0 ? batchSizeParsed : 500

    const maxRuntimeRaw = this.configService.get<string>('RUSHDB_EMBEDDING_MAX_RUNTIME_MS')
    const maxRuntimeParsed = Number.parseInt(maxRuntimeRaw ?? '50000', 10)
    const maxRuntimeMs =
      Number.isInteger(maxRuntimeParsed) && maxRuntimeParsed > 0 ? maxRuntimeParsed : 50_000

    await Promise.allSettled(pending.map((index) => this.backfillIndex(index, batchSize, maxRuntimeMs)))
  }

  private async backfillIndex(
    index: {
      id: string
      projectId: string
      propertyName: string
      label: string
      vectorPropertyName: string
      sourceType: string
    },
    batchSize: number,
    maxRuntimeMs: number
  ): Promise<void> {
    if (index.sourceType === 'external') {
      return
    }

    await this.embeddingIndexRepository.updateStatus(index.id, 'indexing')

    const project = await this.projectRepository.findById(index.projectId)
    const workspaceId = project?.workspaceId
    const isExternalDbProject = !!project?.customDb

    // propKey is written to rel.__propKey and used for exact label/property isolation.
    // Format: "Label:propertyName" (e.g. "Book:title").
    const propKey = `${index.label}:${index.propertyName}`
    const labelSuffix = index.label ? `:${index.label}` : ''

    const deadline = Date.now() + maxRuntimeMs
    let skip = 0

    this.logger.log(
      `[backfillIndex] start id=${index.id} label=${index.label} property=${index.propertyName} ` +
        `propKey=${propKey} batchSize=${batchSize} maxRuntimeMs=${maxRuntimeMs}`
    )

    try {
      while (Date.now() < deadline) {
        const session = this.neogmaService.createSession('embedding-backfill')
        let relations: Array<{ relId: string; value: unknown }>

        try {
          const result = await session.run(
            this.aiQueryService.getUnindexedRelationsQuery(labelSuffix, index.vectorPropertyName),
            {
              projectId: index.projectId,
              propertyName: index.propertyName,
              propKey,
              skip: neo4jInt(skip),
              batchSize: neo4jInt(batchSize)
            }
          )

          relations = result.records.map((r) => ({
            relId: r.get('relId') as string,
            value: r.get('value')
          }))
        } finally {
          await session.close()
        }

        this.logger.log(
          `[backfillIndex] id=${index.id} skip=${skip} unindexed relations found=${relations.length}`
        )

        if (relations.length === 0) {
          break
        }

        // Build texts to embed (array values join with space; other types coerce to string)
        const texts = relations.map(({ value }) =>
          Array.isArray(value) ? (value as string[]).join(' ') : String(value)
        )

        if (workspaceId && !isExternalDbProject) {
          const estimatedTokens = estimateBatchTokens(texts)
          try {
            await this.billingPolicyService.assertEmbeddingBatchAllowed(
              workspaceId,
              estimateEmbeddingBatchKu(relations.length, estimatedTokens)
            )
          } catch (error) {
            const reason = error instanceof HttpException ? (error.getResponse() as any)?.message : undefined
            this.logger.warn(
              `[backfillIndex] id=${index.id} paused by billing policy: ${reason ?? 'would exceed allowed KU policy'}`
            )
            await this.embeddingIndexRepository.updateStatus(index.id, 'pending')
            return
          }
        }

        this.logger.debug(
          `[backfillIndex] id=${index.id} sample texts[0..2]: ${JSON.stringify(texts.slice(0, 3))}`
        )

        const { embeddings, tokensUsed } = await this.embeddingProviderService.embedBatchWithUsage(texts)

        this.logger.log(
          `[backfillIndex] id=${index.id} embeddings received=${embeddings.length} ` +
            `dims=${embeddings[0]?.length ?? 0} nulls=${embeddings.filter((e) => e == null).length}`
        )

        const updates = relations
          .map((rel, i) => ({
            relId: rel.relId,
            emb: embeddings[i],
            projectId: index.projectId,
            propKey
          }))
          .filter((u) => u.emb != null)

        this.logger.log(`[backfillIndex] id=${index.id} updates to write=${updates.length}`)

        if (updates.length > 0) {
          const writeSession = this.neogmaService.createSession('embedding-backfill-write')
          try {
            const tx = writeSession.beginTransaction({ timeout: 30_000 })
            try {
              await tx.run(this.aiQueryService.getWriteEmbeddingsQuery(index.vectorPropertyName), {
                updates
              })
              await tx.commit()
              this.logger.log(`[backfillIndex] id=${index.id} committed ${updates.length} embedding writes`)
            } catch (err) {
              await tx.rollback()
              this.logger.error(`[backfillIndex] id=${index.id} tx rollback: ${err}`)
              throw err
            }
          } finally {
            await writeSession.close()
          }

          if (workspaceId && !isExternalDbProject) {
            const tokenCount = tokensUsed ?? estimateBatchTokens(texts)
            this.kuEventsService.emit(workspaceId, index.projectId, KuOperation.EMBEDDING_GENERATED, {
              count: updates.length,
              tokenCount,
              estimatedTokens: tokenCount,
              source: 'backfill_batch',
              indexId: index.id,
              label: index.label,
              propertyName: index.propertyName
            })
          }
        }

        if (relations.length < batchSize) {
          break
        }
        skip += batchSize
      }

      // Check if any unindexed remain (timed out case)
      const checkSession = this.neogmaService.createSession('embedding-backfill-check')
      let remaining = 0
      try {
        const result = await checkSession.run(
          this.aiQueryService.getUnindexedCountQuery(labelSuffix, index.vectorPropertyName),
          {
            projectId: index.projectId,
            propertyName: index.propertyName,
            propKey
          }
        )
        const raw = result.records[0]?.get('remaining')
        remaining = typeof raw?.toNumber === 'function' ? raw.toNumber() : Number(raw ?? 0)
      } finally {
        await checkSession.close()
      }

      const finalStatus = remaining === 0 ? 'ready' : 'pending'
      this.logger.log(`[backfillIndex] id=${index.id} done remaining=${remaining} status=${finalStatus}`)
      await this.embeddingIndexRepository.updateStatus(index.id, finalStatus)
    } catch (err) {
      this.logger.error(`[EmbeddingBackfill] index ${index.id} failed: ${err}`)
      await this.embeddingIndexRepository.updateStatus(index.id, 'error')
    }
  }
}
