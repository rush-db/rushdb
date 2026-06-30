import { Injectable } from '@nestjs/common'
import { desc, eq } from 'drizzle-orm'

import { SqlService } from '@/database/sql/sql.service'

import type { InsertSavedQueryRow, SavedQueryRow } from '@/database/sql/schema/types'

@Injectable()
export class SavedQueryRepository {
  constructor(private readonly sql: SqlService) {}

  private get db() {
    return this.sql.db
  }

  private get savedQueries() {
    return this.sql.tables.savedQueries
  }

  async create(data: InsertSavedQueryRow): Promise<SavedQueryRow> {
    await this.db.insert(this.savedQueries).values(data)
    return this.findById(data.id)
  }

  async findById(id: string): Promise<SavedQueryRow | undefined> {
    const rows = await this.db.select().from(this.savedQueries).where(eq(this.savedQueries.id, id))
    return rows[0]
  }

  async findByProjectId(projectId: string): Promise<SavedQueryRow[]> {
    return this.db
      .select()
      .from(this.savedQueries)
      .where(eq(this.savedQueries.projectId, projectId))
      .orderBy(desc(this.savedQueries.createdAt))
  }

  async update(
    id: string,
    data: Partial<Omit<InsertSavedQueryRow, 'id'>>
  ): Promise<SavedQueryRow | undefined> {
    await this.db.update(this.savedQueries).set(data).where(eq(this.savedQueries.id, id))
    return this.findById(id)
  }

  async delete(id: string): Promise<boolean> {
    await this.db.delete(this.savedQueries).where(eq(this.savedQueries.id, id))
    return true
  }
}
