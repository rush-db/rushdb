import { Injectable } from '@nestjs/common'
import { and, eq } from 'drizzle-orm'

import { SqlService } from '@/database/sql/sql.service'
import type { InsertUserRow, UserRow } from '@/database/sql/schema/types'

@Injectable()
export class UserRepository {
  constructor(private readonly sql: SqlService) {}

  private get db() {
    return this.sql.db
  }
  private get users() {
    return this.sql.tables.users
  }
  private get workspaceMembers() {
    return this.sql.tables.workspaceMembers
  }
  private get workspaces() {
    return this.sql.tables.workspaces
  }
  private get projectAccess() {
    return this.sql.tables.projectAccess
  }
  private get projects() {
    return this.sql.tables.projects
  }

  async findById(id: string): Promise<UserRow | undefined> {
    const rows = await this.db.select().from(this.users).where(eq(this.users.id, id))
    return rows[0]
  }

  async findByLogin(login: string): Promise<UserRow | undefined> {
    const rows = await this.db.select().from(this.users).where(eq(this.users.login, login))
    return rows[0]
  }

  async create(data: InsertUserRow): Promise<UserRow> {
    await this.db.insert(this.users).values(data)
    return this.findById(data.id)
  }

  async update(id: string, data: Partial<Omit<InsertUserRow, 'id'>>): Promise<UserRow | undefined> {
    await this.db.update(this.users).set(data).where(eq(this.users.id, id))
    return this.findById(id)
  }

  async delete(id: string): Promise<void> {
    await this.db.delete(this.users).where(eq(this.users.id, id))
  }

  async findWorkspacesForUser(
    userId: string
  ): Promise<Array<{ workspace: any; role: string; since: string }>> {
    const rows = await this.db
      .select({
        workspace: this.workspaces,
        role: this.workspaceMembers.role,
        since: this.workspaceMembers.since
      })
      .from(this.workspaceMembers)
      .innerJoin(this.workspaces, eq(this.workspaceMembers.workspaceId, this.workspaces.id))
      .where(eq(this.workspaceMembers.userId, userId))
    return rows
  }

  async findProjectsForUser(userId: string): Promise<any[]> {
    return this.db
      .select({ project: this.projects, role: this.projectAccess.role })
      .from(this.projectAccess)
      .innerJoin(this.projects, eq(this.projectAccess.projectId, this.projects.id))
      .where(eq(this.projectAccess.userId, userId))
  }

  async getUserRoleInWorkspace(userId: string, workspaceId: string): Promise<string | undefined> {
    const rows = await this.db
      .select({ role: this.workspaceMembers.role })
      .from(this.workspaceMembers)
      .where(
        and(eq(this.workspaceMembers.userId, userId), eq(this.workspaceMembers.workspaceId, workspaceId))
      )
    return rows[0]?.role
  }

  async getUserRoleInProject(userId: string, projectId: string): Promise<string | undefined> {
    const rows = await this.db
      .select({ role: this.projectAccess.role })
      .from(this.projectAccess)
      .where(and(eq(this.projectAccess.userId, userId), eq(this.projectAccess.projectId, projectId)))
    return rows[0]?.role
  }
}
