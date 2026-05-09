import { Injectable } from '@nestjs/common'
import { and, eq, inArray, or } from 'drizzle-orm'

import { SqlService } from '@/database/sql/sql.service'

import type {
  EmbeddingIndexSimilarityFunction,
  EmbeddingIndexSourceType
} from '@/core/ai/embedding-index.utils'
import type { EmbeddingIndexRow, InsertEmbeddingIndexRow } from '@/database/sql/schema/types'

@Injectable()
export class EmbeddingIndexRepository {
  constructor(private readonly sql: SqlService) {}

  private get db() {
    return this.sql.db
  }

  private get table() {
    return this.sql.tables.embeddingIndexes
  }

  async findById(id: string): Promise<EmbeddingIndexRow | undefined> {
    const rows = await this.db.select().from(this.table).where(eq(this.table.id, id))
    return rows[0]
  }

  async findByProjectId(projectId: string): Promise<EmbeddingIndexRow[]> {
    return this.db.select().from(this.table).where(eq(this.table.projectId, projectId))
  }

  async findByProjectIdPropertyAndLabel(
    projectId: string,
    propertyName: string,
    label: string,
    signature?: {
      sourceType?: EmbeddingIndexSourceType
      similarityFunction?: EmbeddingIndexSimilarityFunction
      dimensions?: number
    }
  ): Promise<EmbeddingIndexRow | undefined> {
    const conditions = [
      eq(this.table.projectId, projectId),
      eq(this.table.propertyName, propertyName),
      eq(this.table.label, label)
    ]

    if (signature?.sourceType) {
      conditions.push(eq(this.table.sourceType, signature.sourceType))
    }
    if (signature?.similarityFunction) {
      conditions.push(eq(this.table.similarityFunction, signature.similarityFunction))
    }
    if (typeof signature?.dimensions === 'number') {
      conditions.push(eq(this.table.dimensions, signature.dimensions))
    }

    const rows = await this.db
      .select()
      .from(this.table)
      .where(and(...conditions))
    return rows[0]
  }

  async create(data: InsertEmbeddingIndexRow): Promise<EmbeddingIndexRow> {
    await this.db.insert(this.table).values(data)
    return this.findById(data.id)
  }

  async updateStatus(id: string, status: string): Promise<void> {
    const now = new Date().toISOString()
    await this.db.update(this.table).set({ status, updatedAt: now }).where(eq(this.table.id, id))
  }

  /** Returns all enabled embedding index policies that are in 'pending' or 'indexing' status. */
  async findPending(): Promise<EmbeddingIndexRow[]> {
    return this.db
      .select()
      .from(this.table)
      .where(and(eq(this.table.enabled, true), inArray(this.table.status, ['pending', 'indexing'])))
  }

  /**
   * Marks indexes for the given (label, propertyName) pairs as 'pending' so the backfill
   * scheduler picks them up on the next cycle.
   * Only affects enabled indexes that are already in 'ready' or 'error' state.
   */
  async markPendingForProperties(
    projectId: string,
    entries: Array<{ propertyName: string; label: string }>
  ): Promise<void> {
    if (entries.length === 0) {
      return
    }
    const now = new Date().toISOString()

    const pairConditions = entries.map((e) =>
      and(eq(this.table.propertyName, e.propertyName), eq(this.table.label, e.label))
    )

    await this.db
      .update(this.table)
      .set({ status: 'pending', updatedAt: now })
      .where(
        and(
          eq(this.table.projectId, projectId),
          eq(this.table.enabled, true),
          eq(this.table.sourceType, 'managed' as EmbeddingIndexSourceType),
          or(...pairConditions)
        )
      )
  }

  async delete(id: string): Promise<void> {
    await this.db.delete(this.table).where(eq(this.table.id, id))
  }

  async deleteByProjectId(projectId: string): Promise<void> {
    await this.db.delete(this.table).where(eq(this.table.projectId, projectId))
  }

  async countByVectorPropertyName(vectorPropertyName: string): Promise<number> {
    const rows = await this.db
      .select()
      .from(this.table)
      .where(eq(this.table.vectorPropertyName, vectorPropertyName))
    return rows.length
  }

  /**
   * Finds all external embedding indexes matching the given (projectId, label, propertyName, dimensions).
   * Optionally narrows by similarityFunction when provided.
   * Used for inline vector resolution during create/upsert/set.
   */
  async findMatchingExternalIndexes(
    projectId: string,
    label: string,
    propertyName: string,
    dimensions: number,
    similarityFunction?: EmbeddingIndexSimilarityFunction
  ): Promise<EmbeddingIndexRow[]> {
    const conditions = [
      eq(this.table.projectId, projectId),
      eq(this.table.label, label),
      eq(this.table.propertyName, propertyName),
      eq(this.table.sourceType, 'external' as EmbeddingIndexSourceType),
      eq(this.table.dimensions, dimensions)
    ]

    if (similarityFunction) {
      conditions.push(eq(this.table.similarityFunction, similarityFunction))
    }

    return this.db
      .select()
      .from(this.table)
      .where(and(...conditions))
  }
}
