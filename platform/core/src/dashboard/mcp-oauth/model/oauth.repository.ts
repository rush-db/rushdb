import { Injectable } from '@nestjs/common'
import { and, eq, inArray, isNull, notInArray } from 'drizzle-orm'
import { uuidv7 } from 'uuidv7'

import { SqlService } from '@/database/sql/sql.service'

import type {
  InsertOauthAuthRequestRow,
  InsertOauthClientRow,
  InsertOauthCodeRow,
  InsertOauthConsentRow,
  InsertOauthRefreshTokenRow,
  OauthAuthRequestRow,
  OauthClientRow,
  OauthCodeRow,
  OauthConsentRow,
  OauthRefreshTokenRow
} from '@/database/sql/schema/types'

@Injectable()
export class OAuthRepository {
  constructor(private readonly sql: SqlService) {}

  private get db() {
    return this.sql.db
  }
  private get clients() {
    return this.sql.tables.oauthClients
  }
  private get authRequests() {
    return this.sql.tables.oauthAuthRequests
  }
  private get consents() {
    return this.sql.tables.oauthConsents
  }
  private get codes() {
    return this.sql.tables.oauthCodes
  }
  private get refreshTokens() {
    return this.sql.tables.oauthRefreshTokens
  }

  // ── Clients ──────────────────────────────────────────────────────────────────

  /**
   * Upsert an OAuth client keyed on (clientName, redirectUris).
   * Returns the existing or newly created clientId.
   */
  async upsertClient(
    data: Pick<
      InsertOauthClientRow,
      'clientName' | 'redirectUris' | 'tokenEndpointAuthMethod' | 'applicationType'
    >
  ): Promise<string> {
    const existing = await this.db
      .select({ id: this.clients.id })
      .from(this.clients)
      .where(
        and(eq(this.clients.clientName, data.clientName), eq(this.clients.redirectUris, data.redirectUris))
      )
      .limit(1)

    if (existing[0]) {
      // Update optional fields
      await this.db
        .update(this.clients)
        .set({
          tokenEndpointAuthMethod: data.tokenEndpointAuthMethod,
          applicationType: data.applicationType
        })
        .where(eq(this.clients.id, existing[0].id))
      return existing[0].id
    }

    const id = `oauthc_${uuidv7()}`
    await this.db.insert(this.clients).values({
      id,
      clientName: data.clientName,
      redirectUris: data.redirectUris,
      tokenEndpointAuthMethod: data.tokenEndpointAuthMethod,
      applicationType: data.applicationType,
      created: new Date().toISOString()
    })
    return id
  }

  async findClientById(id: string): Promise<OauthClientRow | undefined> {
    const rows = await this.db.select().from(this.clients).where(eq(this.clients.id, id))
    return rows[0]
  }

  // ── Auth Requests ─────────────────────────────────────────────────────────────

  async createAuthRequest(data: InsertOauthAuthRequestRow): Promise<void> {
    await this.db.insert(this.authRequests).values(data)
  }

  async findAuthRequest(id: string): Promise<OauthAuthRequestRow | undefined> {
    const rows = await this.db.select().from(this.authRequests).where(eq(this.authRequests.id, id))
    return rows[0]
  }

  async deleteAuthRequest(id: string): Promise<void> {
    await this.db.delete(this.authRequests).where(eq(this.authRequests.id, id))
  }

  async deleteExpiredAuthRequests(): Promise<number> {
    const now = new Date().toISOString()
    const expired = await this.db
      .select({ id: this.authRequests.id })
      .from(this.authRequests)
      .where(and(eq(this.authRequests.expiresAt, this.authRequests.expiresAt)))
    // Use raw comparison since Drizzle doesn't have lt for text ISO dates natively
    const expiredIds = (
      await this.db
        .select({ id: this.authRequests.id, expiresAt: this.authRequests.expiresAt })
        .from(this.authRequests)
    )
      .filter((r) => r.expiresAt < now)
      .map((r) => r.id)

    for (const id of expiredIds) {
      await this.db.delete(this.authRequests).where(eq(this.authRequests.id, id))
    }
    return expiredIds.length
  }

  // ── Consents ──────────────────────────────────────────────────────────────────

  async findActiveConsent(
    userId: string,
    clientId: string,
    projectId: string
  ): Promise<OauthConsentRow | undefined> {
    const rows = await this.db
      .select()
      .from(this.consents)
      .where(
        and(
          eq(this.consents.userId, userId),
          eq(this.consents.clientId, clientId),
          eq(this.consents.projectId, projectId),
          isNull(this.consents.revokedAt)
        )
      )
      .limit(1)
    return rows[0]
  }

  async createConsent(data: InsertOauthConsentRow): Promise<void> {
    await this.db.insert(this.consents).values(data)
  }

  async findConsentById(id: string): Promise<OauthConsentRow | undefined> {
    const rows = await this.db.select().from(this.consents).where(eq(this.consents.id, id))
    return rows[0]
  }

  async listActiveConsents(userId: string): Promise<OauthConsentRow[]> {
    return this.db
      .select()
      .from(this.consents)
      .where(and(eq(this.consents.userId, userId), isNull(this.consents.revokedAt)))
  }

  async listActiveConsentsWithDetails(
    userId: string,
    workspaceId?: string
  ): Promise<
    {
      id: string
      clientId: string
      clientName: string | null
      projectId: string
      projectName: string | null
      scope: string
      resource: string | null
      created: string
    }[]
  > {
    const projectsTable = this.sql.tables.projects
    const conditions = [
      eq(this.consents.userId, userId),
      isNull(this.consents.revokedAt),
      ...(workspaceId ? [eq(projectsTable.workspaceId, workspaceId)] : [])
    ]
    return this.db
      .select({
        id: this.consents.id,
        clientId: this.consents.clientId,
        clientName: this.clients.clientName,
        projectId: this.consents.projectId,
        projectName: projectsTable.name,
        scope: this.consents.scope,
        resource: this.consents.resource,
        created: this.consents.created
      })
      .from(this.consents)
      .leftJoin(this.clients, eq(this.consents.clientId, this.clients.id))
      .leftJoin(projectsTable, eq(this.consents.projectId, projectsTable.id))
      .where(and(...conditions))
  }

  async revokeConsent(consentId: string): Promise<void> {
    await this.db
      .update(this.consents)
      .set({ revokedAt: new Date().toISOString() })
      .where(eq(this.consents.id, consentId))
  }

  /**
   * Hard-delete all consents for the given project IDs.
   * Called when projects (and their parent workspace) are deleted.
   */
  async deleteConsentsByProjectIds(projectIds: string[]): Promise<void> {
    if (!projectIds.length) {
      return
    }
    await this.db.delete(this.consents).where(inArray(this.consents.projectId, projectIds))
  }

  /**
   * Delete oauth_clients that are no longer referenced by any consent row.
   * Safe to call after deleteConsentsByProjectIds to avoid accumulating orphans.
   */
  async deleteOrphanedClients(): Promise<void> {
    const used = await this.db.selectDistinct({ clientId: this.consents.clientId }).from(this.consents)
    const usedIds: string[] = used.map((r: { clientId: string }) => r.clientId)
    if (usedIds.length > 0) {
      await this.db.delete(this.clients).where(notInArray(this.clients.id, usedIds))
    } else {
      await this.db.delete(this.clients)
    }
  }

  // ── Codes ─────────────────────────────────────────────────────────────────────

  async createCode(data: InsertOauthCodeRow): Promise<void> {
    await this.db.insert(this.codes).values(data)
  }

  async findCode(id: string): Promise<OauthCodeRow | undefined> {
    const rows = await this.db.select().from(this.codes).where(eq(this.codes.id, id))
    return rows[0]
  }

  async deleteCode(id: string): Promise<void> {
    await this.db.delete(this.codes).where(eq(this.codes.id, id))
  }

  async deleteExpiredCodes(): Promise<number> {
    const now = new Date().toISOString()
    const expiredIds = (
      await this.db.select({ id: this.codes.id, expiresAt: this.codes.expiresAt }).from(this.codes)
    )
      .filter((r) => r.expiresAt < now)
      .map((r) => r.id)

    for (const id of expiredIds) {
      await this.db.delete(this.codes).where(eq(this.codes.id, id))
    }
    return expiredIds.length
  }

  // ── Refresh Tokens ────────────────────────────────────────────────────────────

  /**
   * Persists a refresh token record. The `id` field MUST be the SHA-256 hex
   * digest of the raw token — the raw value is never stored.
   */
  async createRefreshToken(data: InsertOauthRefreshTokenRow): Promise<void> {
    await this.db.insert(this.refreshTokens).values(data)
  }

  /**
   * Looks up a refresh token by its hashed ID (SHA-256 hex of the raw value).
   */
  async findRefreshToken(hashedId: string): Promise<OauthRefreshTokenRow | undefined> {
    const rows = await this.db.select().from(this.refreshTokens).where(eq(this.refreshTokens.id, hashedId))
    return rows[0]
  }

  /**
   * Deletes a single refresh token (used for rotation: invalidate before issuing a new one).
   */
  async deleteRefreshToken(hashedId: string): Promise<void> {
    await this.db.delete(this.refreshTokens).where(eq(this.refreshTokens.id, hashedId))
  }

  /**
   * Deletes all refresh tokens tied to a consent (called on consent revocation).
   */
  async deleteRefreshTokensByConsentId(consentId: string): Promise<void> {
    await this.db.delete(this.refreshTokens).where(eq(this.refreshTokens.consentId, consentId))
  }

  /**
   * Removes all expired refresh tokens. Called by the scheduler.
   */
  async deleteExpiredRefreshTokens(): Promise<number> {
    const now = new Date().toISOString()
    const expiredIds = (
      await this.db
        .select({ id: this.refreshTokens.id, expiresAt: this.refreshTokens.expiresAt })
        .from(this.refreshTokens)
    )
      .filter((r) => r.expiresAt < now)
      .map((r) => r.id)

    for (const id of expiredIds) {
      await this.db.delete(this.refreshTokens).where(eq(this.refreshTokens.id, id))
    }
    return expiredIds.length
  }
}
