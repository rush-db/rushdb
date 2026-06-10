import { Injectable, Logger } from '@nestjs/common'

import { AiQueryService } from '@/core/ai/ai-query.service'
import {
  buildVectorIndexName,
  buildVectorPropertyName,
  type EmbeddingIndexSimilarityFunction,
  type EmbeddingIndexSourceType
} from '@/core/ai/embedding-index.utils'
import { NeogmaService } from '@/database/neogma/neogma.service'

export interface VectorIndexSlot {
  sourceType: EmbeddingIndexSourceType
  similarityFunction: EmbeddingIndexSimilarityFunction
  dimensions: number
}

/**
 * Owns Neo4j vector index DDL for embedding index slots.
 *
 * DDL acquires the exclusive schema lock and can block for a long time behind
 * concurrent write transactions (backfill batches, imports), so callers on the
 * HTTP request path must NOT await it — fire-and-forget and rely on the backfill
 * scheduler calling ensureVectorIndexExists() as the durable retry.
 *
 * Successfully created slots are cached per process so the scheduler doesn't
 * re-issue DDL every minute. The cache is invalidated on dropVectorIndex().
 */
@Injectable()
export class EmbeddingIndexDdlService {
  private readonly logger = new Logger(EmbeddingIndexDdlService.name)
  private readonly ensuredIndexNames = new Set<string>()

  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly aiQueryService: AiQueryService
  ) {}

  async ensureVectorIndexExists(slot: VectorIndexSlot): Promise<void> {
    const indexName = buildVectorIndexName(slot)
    if (this.ensuredIndexNames.has(indexName)) {
      return
    }

    const session = this.neogmaService.createSession('embedding-index-ddl')
    try {
      await session.run(
        this.aiQueryService.getCreateVectorIndexQuery({
          indexName,
          vectorPropertyName: buildVectorPropertyName(slot),
          similarityFunction: slot.similarityFunction
        }),
        { dimensions: slot.dimensions }
      )
      this.ensuredIndexNames.add(indexName)
      this.logger.log(`vector index ensured: ${indexName}`)
    } finally {
      await session.close()
    }
  }

  async dropVectorIndex(indexName: string): Promise<void> {
    this.ensuredIndexNames.delete(indexName)

    const session = this.neogmaService.createSession('embedding-index-ddl')
    try {
      await session.run(this.aiQueryService.getDropVectorIndexQuery(indexName))
      this.logger.log(`vector index dropped: ${indexName}`)
    } finally {
      await session.close()
    }
  }
}
