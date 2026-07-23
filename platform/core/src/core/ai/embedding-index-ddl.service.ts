import { Injectable, Logger } from '@nestjs/common'
import { int as neo4jInt } from 'neo4j-driver'

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
  private readonly onlineIndexNames = new Set<string>()
  private readonly inflightDdl = new Map<string, Promise<void>>()

  constructor(
    private readonly neogmaService: NeogmaService,
    private readonly aiQueryService: AiQueryService
  ) {}

  async ensureVectorIndexExists(slot: VectorIndexSlot): Promise<void> {
    const indexName = buildVectorIndexName(slot)
    if (this.ensuredIndexNames.has(indexName)) {
      return
    }

    // A CREATE queued on the schema lock can hold its session for minutes; concurrent
    // callers (scheduler ticks, searches) share that one attempt instead of stacking
    // more sessions into the same lock queue.
    const inflight = this.inflightDdl.get(indexName)
    if (inflight) {
      return inflight
    }

    const attempt = (async () => {
      const session = this.neogmaService.createSession('embedding-index-ddl')
      try {
        await session.run(
          this.aiQueryService.getCreateVectorIndexQuery({
            indexName,
            vectorPropertyName: buildVectorPropertyName(slot),
            similarityFunction: slot.similarityFunction
          }),
          { dimensions: neo4jInt(slot.dimensions) }
        )
        this.ensuredIndexNames.add(indexName)
        this.logger.log(`vector index ensured: ${indexName}`)
      } finally {
        await session.close()
      }
    })()

    this.inflightDdl.set(indexName, attempt)
    try {
      await attempt
    } finally {
      this.inflightDdl.delete(indexName)
    }
  }

  /**
   * Lock-free readiness probe for the request path: a SHOW INDEXES schema read that
   * never queues behind the exclusive schema lock. Returns true only once the index
   * is ONLINE — an existing-but-populating index cannot serve vector queries yet.
   * ONLINE indexes are cached per process; dropVectorIndex() invalidates the cache.
   */
  async isVectorIndexOnline(slot: VectorIndexSlot): Promise<boolean> {
    const indexName = buildVectorIndexName(slot)
    if (this.onlineIndexNames.has(indexName)) {
      return true
    }

    const session = this.neogmaService.createSession('embedding-index-ddl')
    try {
      const result = await session.run(this.aiQueryService.getVectorIndexStateQuery(), { indexName })
      const state = result.records[0]?.get('state')
      if (state === 'ONLINE') {
        this.onlineIndexNames.add(indexName)
        return true
      }
      return false
    } finally {
      await session.close()
    }
  }

  async dropVectorIndex(indexName: string): Promise<void> {
    this.ensuredIndexNames.delete(indexName)
    this.onlineIndexNames.delete(indexName)

    const session = this.neogmaService.createSession('embedding-index-ddl')
    try {
      await session.run(this.aiQueryService.getDropVectorIndexQuery(indexName))
      this.logger.log(`vector index dropped: ${indexName}`)
    } finally {
      await session.close()
    }
  }
}
