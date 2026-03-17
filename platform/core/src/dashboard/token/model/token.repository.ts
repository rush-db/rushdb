import { Injectable } from '@nestjs/common'
import { and, eq, gt, isNull, ne, or, sql } from 'drizzle-orm'

import { SqlService } from '@/database/sql/sql.service'
import type { InsertTokenRow, TokenRow } from '@/database/sql/schema/types'

export interface TokenWithProjectAndWorkspace {
  token: TokenRow
  project: any
  workspace: any
}

@Injectable()
export class TokenRepository {
  constructor(private readonly sql: SqlService) {}

  private get db() {
    return this.sql.db
  }
  private get tokens() {
    return this.sql.tables.tokens
  }
  private get projects() {
    return this.sql.tables.projects
  }
  private get workspaces() {
    return this.sql.tables.workspaces
  }

  async findById(id: string): Promise<TokenRow | undefined> {
    const rows = await this.db.select().from(this.tokens).where(eq(this.tokens.id, id))
    return rows[0]
  }

  async create(data: InsertTokenRow): Promise<TokenRow> {
    await this.db.insert(this.tokens).values(data)
    return this.findById(data.id)
  }

  async delete(id: string): Promise<boolean> {
    const rows = await this.db.select({ id: this.tokens.id }).from(this.tokens).where(eq(this.tokens.id, id))
    if (rows.length === 0) return false
    await this.db.delete(this.tokens).where(eq(this.tokens.id, id))
    return true
  }

  async findByProjectId(projectId: string): Promise<Array<TokenRow>> {
    return this.db.select().from(this.tokens).where(eq(this.tokens.projectId, projectId))
  }

  async findByConsentId(consentId: string): Promise<TokenRow | undefined> {
    const rows = await this.db.select().from(this.tokens).where(eq(this.tokens.consentId, consentId))
    return rows[0]
  }

  /**
   * Find a live token for the given consent+project combo (used for dedup in OAuth token exchange).
   * Live = expiration is -1 OR (created + expiration > now)
   */
  async findLiveTokenByConsentAndProject(
    consentId: string,
    projectId: string
  ): Promise<TokenRow | undefined> {
    const nowMs = Date.now()
    const epochExpr =
      this.sql.isPostgres ?
        sql`(EXTRACT(EPOCH FROM ${this.tokens.created}::timestamptz) * 1000 + ${this.tokens.expiration})`
      : sql`(strftime('%s', ${this.tokens.created}) * 1000 + ${this.tokens.expiration})`
    const rows = await this.db
      .select()
      .from(this.tokens)
      .where(
        and(
          eq(this.tokens.consentId, consentId),
          eq(this.tokens.projectId, projectId),
          or(eq(this.tokens.expiration, -1), sql`${epochExpr} > ${nowMs}`)
        )
      )
      .limit(1)
    return rows[0]
  }

  /** Traverse token → project → workspace in one query (replaces Cypher traversal). */
  async findTokenWithProjectAndWorkspace(tokenId: string): Promise<TokenWithProjectAndWorkspace | undefined> {
    const rows = await this.db
      .select({
        token: this.tokens,
        project: this.projects,
        workspace: this.workspaces
      })
      .from(this.tokens)
      .innerJoin(this.projects, eq(this.tokens.projectId, this.projects.id))
      .innerJoin(this.workspaces, eq(this.projects.workspaceId, this.workspaces.id))
      .where(eq(this.tokens.id, tokenId))
    return rows[0]
  }

  async deleteByConsentId(consentId: string): Promise<void> {
    await this.db.delete(this.tokens).where(eq(this.tokens.consentId, consentId))
  }

  async deleteExpired(): Promise<number> {
    const nowMs = Date.now()
    const epochExpr =
      this.sql.isPostgres ?
        sql`(EXTRACT(EPOCH FROM ${this.tokens.created}::timestamptz) * 1000 + ${this.tokens.expiration})`
      : sql`(strftime('%s', ${this.tokens.created}) * 1000 + ${this.tokens.expiration})`
    const expired = await this.db
      .select({ id: this.tokens.id })
      .from(this.tokens)
      .where(and(ne(this.tokens.expiration, -1), sql`${epochExpr} < ${nowMs}`))
    if (expired.length === 0) return 0
    for (const { id } of expired) {
      await this.db.delete(this.tokens).where(eq(this.tokens.id, id))
    }
    return expired.length
  }
}
