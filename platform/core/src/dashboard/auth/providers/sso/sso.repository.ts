import { Injectable } from '@nestjs/common'
import { and, eq } from 'drizzle-orm'

import { SqlService } from '@/database/sql/sql.service'

import type {
  InsertWorkspaceIdentityProviderRow,
  WorkspaceIdentityProviderRow
} from '@/database/sql/schema/types'

@Injectable()
export class SsoRepository {
  constructor(private readonly sql: SqlService) {}

  private get db() {
    return this.sql.db
  }
  private get table() {
    return this.sql.tables.workspaceIdentityProviders
  }

  async findById(id: string): Promise<WorkspaceIdentityProviderRow | undefined> {
    const rows = await this.db.select().from(this.table).where(eq(this.table.id, id))
    return rows[0]
  }

  async findByWorkspaceId(workspaceId: string): Promise<WorkspaceIdentityProviderRow[]> {
    return this.db.select().from(this.table).where(eq(this.table.workspaceId, workspaceId))
  }

  async findByWorkspaceAndType(
    workspaceId: string,
    type: string
  ): Promise<WorkspaceIdentityProviderRow | undefined> {
    const rows = await this.db
      .select()
      .from(this.table)
      .where(and(eq(this.table.workspaceId, workspaceId), eq(this.table.type, type)))
    return rows[0]
  }

  async findAllEnabled(): Promise<WorkspaceIdentityProviderRow[]> {
    return this.db.select().from(this.table).where(eq(this.table.enabled, true))
  }

  async create(data: InsertWorkspaceIdentityProviderRow): Promise<WorkspaceIdentityProviderRow> {
    await this.db.insert(this.table).values(data)
    return this.findById(data.id)
  }

  async update(
    id: string,
    data: Partial<Omit<InsertWorkspaceIdentityProviderRow, 'id'>>
  ): Promise<WorkspaceIdentityProviderRow | undefined> {
    await this.db.update(this.table).set(data).where(eq(this.table.id, id))
    return this.findById(id)
  }

  async delete(id: string): Promise<void> {
    await this.db.delete(this.table).where(eq(this.table.id, id))
  }
}
