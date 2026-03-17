import { Injectable } from '@nestjs/common'
import { and, eq } from 'drizzle-orm'

import { SqlService } from '@/database/sql/sql.service'
import type {
  InsertWorkspaceInviteRow,
  InsertWorkspaceMemberRow,
  InsertWorkspaceRow,
  WorkspaceInviteRow,
  WorkspaceMemberRow,
  WorkspaceRow
} from '@/database/sql/schema/types'

@Injectable()
export class WorkspaceRepository {
  constructor(private readonly sql: SqlService) {}

  private get db() {
    return this.sql.db
  }
  private get workspaces() {
    return this.sql.tables.workspaces
  }
  private get workspaceMembers() {
    return this.sql.tables.workspaceMembers
  }
  private get workspaceInvites() {
    return this.sql.tables.workspaceInvites
  }
  private get projects() {
    return this.sql.tables.projects
  }
  private get users() {
    return this.sql.tables.users
  }

  async findById(id: string): Promise<WorkspaceRow | undefined> {
    const rows = await this.db.select().from(this.workspaces).where(eq(this.workspaces.id, id))
    return rows[0]
  }

  async findAll(): Promise<WorkspaceRow[]> {
    return this.db.select().from(this.workspaces)
  }

  async create(data: InsertWorkspaceRow): Promise<WorkspaceRow> {
    await this.db.insert(this.workspaces).values(data)
    return this.findById(data.id)
  }

  async update(id: string, data: Partial<Omit<InsertWorkspaceRow, 'id'>>): Promise<WorkspaceRow | undefined> {
    await this.db.update(this.workspaces).set(data).where(eq(this.workspaces.id, id))
    return this.findById(id)
  }

  async delete(id: string): Promise<void> {
    await this.db.delete(this.workspaces).where(eq(this.workspaces.id, id))
  }

  async findByProjectId(projectId: string): Promise<WorkspaceRow | undefined> {
    const rows = await this.db
      .select({ workspace: this.workspaces })
      .from(this.projects)
      .innerJoin(this.workspaces, eq(this.projects.workspaceId, this.workspaces.id))
      .where(eq(this.projects.id, projectId))
    return rows[0]?.workspace
  }

  // ── Members ──────────────────────────────────────────────────────────────────

  async addMember(data: InsertWorkspaceMemberRow): Promise<void> {
    await this.db
      .insert(this.workspaceMembers)
      .values(data)
      .onConflictDoUpdate({
        target: [this.workspaceMembers.workspaceId, this.workspaceMembers.userId],
        set: { role: data.role, since: data.since }
      })
  }

  async removeMember(workspaceId: string, userId: string): Promise<void> {
    await this.db
      .delete(this.workspaceMembers)
      .where(
        and(eq(this.workspaceMembers.workspaceId, workspaceId), eq(this.workspaceMembers.userId, userId))
      )
  }

  async getMember(workspaceId: string, userId: string): Promise<WorkspaceMemberRow | undefined> {
    const rows = await this.db
      .select()
      .from(this.workspaceMembers)
      .where(
        and(eq(this.workspaceMembers.workspaceId, workspaceId), eq(this.workspaceMembers.userId, userId))
      )
    return rows[0]
  }

  async getMembers(workspaceId: string): Promise<Array<{ id: string; login: string; role: string }>> {
    const rows = await this.db
      .select({
        id: this.users.id,
        login: this.users.login,
        role: this.workspaceMembers.role
      })
      .from(this.workspaceMembers)
      .innerJoin(this.users, eq(this.workspaceMembers.userId, this.users.id))
      .where(eq(this.workspaceMembers.workspaceId, workspaceId))
    return rows
  }

  async getMembersWithOwnerCount(
    workspaceId: string,
    excludeUserId: string
  ): Promise<{
    ownerOther: number
    developerOther: number
  }> {
    // Count owns/editor roles of the user outside this workspace
    const allMemberships = await this.db
      .select({ workspaceId: this.workspaceMembers.workspaceId, role: this.workspaceMembers.role })
      .from(this.workspaceMembers)
      .where(eq(this.workspaceMembers.userId, excludeUserId))

    const ownerOther = allMemberships.filter(
      (m) => m.role === 'owner' && m.workspaceId !== workspaceId
    ).length
    const developerOther = allMemberships.filter(
      (m) => m.role !== 'owner' && m.workspaceId !== workspaceId
    ).length

    return { ownerOther, developerOther }
  }

  // ── Invites ───────────────────────────────────────────────────────────────────

  async getInvites(workspaceId: string): Promise<WorkspaceInviteRow[]> {
    return this.db
      .select()
      .from(this.workspaceInvites)
      .where(eq(this.workspaceInvites.workspaceId, workspaceId))
  }

  async addInvite(data: InsertWorkspaceInviteRow): Promise<void> {
    await this.db.insert(this.workspaceInvites).values(data)
  }

  async removeInvite(workspaceId: string, email: string): Promise<void> {
    await this.db
      .delete(this.workspaceInvites)
      .where(and(eq(this.workspaceInvites.workspaceId, workspaceId), eq(this.workspaceInvites.email, email)))
  }
}
