import { Injectable } from '@nestjs/common'
import { and, eq, inArray, lte } from 'drizzle-orm'
import { uuidv7 } from 'uuidv7'

import { SqlService } from '@/database/sql/sql.service'

import type {
  InsertRelationshipAnalysisQueueRow,
  InsertRelationshipPatternRow,
  RelationshipAnalysisQueueRow,
  RelationshipPatternRow
} from '@/database/sql/schema/types'

@Injectable()
export class RelationshipPatternsRepository {
  constructor(private readonly sql: SqlService) {}

  private get db() {
    return this.sql.db
  }

  private get patterns() {
    return this.sql.tables.relationshipPatterns
  }

  private get queue() {
    return this.sql.tables.relationshipAnalysisQueue
  }

  async findByProjectId(projectId: string): Promise<RelationshipPatternRow[]> {
    return this.db.select().from(this.patterns).where(eq(this.patterns.projectId, projectId))
  }

  async findById(id: string, projectId?: string): Promise<RelationshipPatternRow | undefined> {
    const where =
      projectId ?
        and(eq(this.patterns.id, id), eq(this.patterns.projectId, projectId))
      : eq(this.patterns.id, id)
    const rows = await this.db.select().from(this.patterns).where(where)
    return rows[0]
  }

  async findApproved(projectId: string): Promise<RelationshipPatternRow[]> {
    return this.db
      .select()
      .from(this.patterns)
      .where(and(eq(this.patterns.projectId, projectId), eq(this.patterns.status, 'approved')))
  }

  async upsertCandidate(
    data: Omit<InsertRelationshipPatternRow, 'id' | 'createdAt' | 'updatedAt'> & {
      id?: string
    }
  ): Promise<RelationshipPatternRow> {
    const now = new Date().toISOString()
    const id = data.id ?? uuidv7()
    const insertData: InsertRelationshipPatternRow = {
      ...data,
      id,
      createdAt: now,
      updatedAt: now
    }

    await this.db
      .insert(this.patterns)
      .values(insertData)
      .onConflictDoUpdate({
        target: [this.patterns.projectId, this.patterns.signatureHash],
        set: {
          sourceLabel: insertData.sourceLabel,
          sourceKey: insertData.sourceKey,
          sourceWhere: insertData.sourceWhere,
          targetLabel: insertData.targetLabel,
          targetKey: insertData.targetKey,
          targetWhere: insertData.targetWhere,
          direction: insertData.direction,
          type: insertData.type,
          mode: insertData.mode,
          confidence: insertData.confidence,
          origin: insertData.origin,
          rationale: insertData.rationale,
          sampleMatchCount: insertData.sampleMatchCount,
          lastAnalyzedAt: insertData.lastAnalyzedAt,
          lastError: null,
          updatedAt: now
        }
      })

    const rows = await this.db
      .select()
      .from(this.patterns)
      .where(
        and(eq(this.patterns.projectId, data.projectId), eq(this.patterns.signatureHash, data.signatureHash))
      )
    return rows[0]
  }

  async updatePattern(
    id: string,
    data: Partial<Omit<InsertRelationshipPatternRow, 'id' | 'projectId' | 'createdAt'>>,
    projectId?: string
  ): Promise<RelationshipPatternRow | undefined> {
    const where =
      projectId ?
        and(eq(this.patterns.id, id), eq(this.patterns.projectId, projectId))
      : eq(this.patterns.id, id)
    await this.db
      .update(this.patterns)
      .set({ ...data, updatedAt: new Date().toISOString() })
      .where(where)
    return this.findById(id, projectId)
  }

  async approveManyByIds(ids: string[], projectId: string): Promise<RelationshipPatternRow[]> {
    if (!ids.length) {
      return []
    }
    const scope = and(eq(this.patterns.projectId, projectId), inArray(this.patterns.id, ids))
    await this.db
      .update(this.patterns)
      .set({
        status: 'approved',
        lastError: null,
        // Cleared so each pattern reads as pending-apply; the background drain reapplies.
        lastAppliedAt: null,
        updatedAt: new Date().toISOString()
      })
      .where(scope)
    return this.db.select().from(this.patterns).where(scope)
  }

  async deletePattern(id: string, projectId?: string): Promise<void> {
    const where =
      projectId ?
        and(eq(this.patterns.id, id), eq(this.patterns.projectId, projectId))
      : eq(this.patterns.id, id)
    await this.db.delete(this.patterns).where(where)
  }

  async getQueue(projectId: string): Promise<RelationshipAnalysisQueueRow | undefined> {
    const rows = await this.db.select().from(this.queue).where(eq(this.queue.projectId, projectId))
    return rows[0]
  }

  async enqueueAnalysis(projectId: string, notBefore: string): Promise<void> {
    const now = new Date().toISOString()
    const data: InsertRelationshipAnalysisQueueRow = {
      projectId,
      requestedAt: now,
      notBefore,
      status: 'pending',
      createdAt: now,
      updatedAt: now
    }

    await this.db
      .insert(this.queue)
      .values(data)
      .onConflictDoUpdate({
        target: this.queue.projectId,
        set: {
          requestedAt: now,
          notBefore,
          status: 'pending',
          lastError: null,
          updatedAt: now
        }
      })
  }

  async findDueAnalysis(nowIso: string, limit = 10): Promise<RelationshipAnalysisQueueRow[]> {
    return this.db
      .select()
      .from(this.queue)
      .where(and(inArray(this.queue.status, ['pending', 'error']), lte(this.queue.notBefore, nowIso)))
      .limit(limit)
  }

  async updateQueue(projectId: string, data: Partial<InsertRelationshipAnalysisQueueRow>): Promise<void> {
    await this.db
      .update(this.queue)
      .set({ ...data, updatedAt: new Date().toISOString() })
      .where(eq(this.queue.projectId, projectId))
  }
}
