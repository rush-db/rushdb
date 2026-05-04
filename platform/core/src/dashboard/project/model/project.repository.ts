import { Injectable } from '@nestjs/common'
import { and, eq, isNull } from 'drizzle-orm'

import { SqlService } from '@/database/sql/sql.service'

import type {
  InsertProjectAccessRow,
  InsertProjectRow,
  ProjectAccessRow,
  ProjectRow
} from '@/database/sql/schema/types'

@Injectable()
export class ProjectRepository {
  constructor(private readonly sql: SqlService) {}

  private get db() {
    return this.sql.db
  }
  private get projects() {
    return this.sql.tables.projects
  }
  private get projectAccess() {
    return this.sql.tables.projectAccess
  }
  private get workspaceMembers() {
    return this.sql.tables.workspaceMembers
  }

  async findById(id: string): Promise<ProjectRow | undefined> {
    const rows = await this.db
      .select()
      .from(this.projects)
      .where(and(eq(this.projects.id, id), isNull(this.projects.deleted)))
    return rows[0]
  }

  async findAllWithoutCustomDb(): Promise<ProjectRow[]> {
    return this.db
      .select()
      .from(this.projects)
      .where(and(isNull(this.projects.deleted), isNull(this.projects.customDb)))
  }

  async findByIdIncludingDeleted(id: string): Promise<ProjectRow | undefined> {
    const rows = await this.db.select().from(this.projects).where(eq(this.projects.id, id))
    return rows[0]
  }

  async create(data: InsertProjectRow): Promise<ProjectRow> {
    await this.db.insert(this.projects).values(data)
    return this.findByIdIncludingDeleted(data.id)
  }

  async update(id: string, data: Partial<Omit<InsertProjectRow, 'id'>>): Promise<ProjectRow | undefined> {
    await this.db.update(this.projects).set(data).where(eq(this.projects.id, id))
    return this.findByIdIncludingDeleted(id)
  }

  async delete(id: string): Promise<void> {
    await this.db.delete(this.projects).where(eq(this.projects.id, id))
  }

  async findByWorkspaceId(workspaceId: string): Promise<ProjectRow[]> {
    return this.db
      .select()
      .from(this.projects)
      .where(and(eq(this.projects.workspaceId, workspaceId), isNull(this.projects.deleted)))
  }

  /** Projects accessible by userId within a workspace (user has project_access + workspace membership). */
  async findForUser(workspaceId: string, userId: string): Promise<ProjectRow[]> {
    return this.db
      .select({ project: this.projects })
      .from(this.projects)
      .innerJoin(
        this.projectAccess,
        and(eq(this.projectAccess.projectId, this.projects.id), eq(this.projectAccess.userId, userId))
      )
      .where(and(eq(this.projects.workspaceId, workspaceId), isNull(this.projects.deleted)))
      .then((rows) => rows.map((r) => r.project))
  }

  // ── Access ────────────────────────────────────────────────────────────────────

  async grantAccess(data: InsertProjectAccessRow): Promise<void> {
    await this.db
      .insert(this.projectAccess)
      .values(data)
      .onConflictDoUpdate({
        target: [this.projectAccess.projectId, this.projectAccess.userId],
        set: { role: data.role, since: data.since }
      })
  }

  async revokeAccess(projectId: string, userId: string): Promise<void> {
    await this.db
      .delete(this.projectAccess)
      .where(and(eq(this.projectAccess.projectId, projectId), eq(this.projectAccess.userId, userId)))
  }

  async getAccess(projectId: string, userId: string): Promise<ProjectAccessRow | undefined> {
    const rows = await this.db
      .select()
      .from(this.projectAccess)
      .where(and(eq(this.projectAccess.projectId, projectId), eq(this.projectAccess.userId, userId)))
    return rows[0]
  }

  async hasAccess(projectId: string, userId: string): Promise<boolean> {
    const row = await this.getAccess(projectId, userId)
    return !!row
  }

  async getAccessListByProjectId(projectId: string, role?: string): Promise<ProjectAccessRow[]> {
    if (role) {
      return this.db
        .select()
        .from(this.projectAccess)
        .where(and(eq(this.projectAccess.projectId, projectId), eq(this.projectAccess.role, role)))
    }
    return this.db.select().from(this.projectAccess).where(eq(this.projectAccess.projectId, projectId))
  }

  async revokeAllProjectAccessForUserInWorkspace(userId: string, workspaceId: string): Promise<void> {
    // Get all project IDs in this workspace first
    const projectIds = await this.db
      .select({ id: this.projects.id })
      .from(this.projects)
      .where(eq(this.projects.workspaceId, workspaceId))
      .then((rows) => rows.map((r) => r.id))

    for (const projectId of projectIds) {
      await this.revokeAccess(projectId, userId)
    }
  }
}
