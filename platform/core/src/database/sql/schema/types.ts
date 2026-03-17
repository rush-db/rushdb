import type { sqliteSchema } from './sqlite.schema'

/**
 * Canonical TypeScript types inferred from the SQLite schema.
 * The PG schema has structurally equivalent columns so these types apply to both.
 */

export type UserRow = typeof sqliteSchema.users.$inferSelect
export type InsertUserRow = typeof sqliteSchema.users.$inferInsert

export type WorkspaceRow = typeof sqliteSchema.workspaces.$inferSelect
export type InsertWorkspaceRow = typeof sqliteSchema.workspaces.$inferInsert

export type WorkspaceMemberRow = typeof sqliteSchema.workspaceMembers.$inferSelect
export type InsertWorkspaceMemberRow = typeof sqliteSchema.workspaceMembers.$inferInsert

export type WorkspaceInviteRow = typeof sqliteSchema.workspaceInvites.$inferSelect
export type InsertWorkspaceInviteRow = typeof sqliteSchema.workspaceInvites.$inferInsert

export type ProjectRow = typeof sqliteSchema.projects.$inferSelect
export type InsertProjectRow = typeof sqliteSchema.projects.$inferInsert

export type ProjectAccessRow = typeof sqliteSchema.projectAccess.$inferSelect
export type InsertProjectAccessRow = typeof sqliteSchema.projectAccess.$inferInsert

export type TokenRow = typeof sqliteSchema.tokens.$inferSelect
export type InsertTokenRow = typeof sqliteSchema.tokens.$inferInsert

export type OauthClientRow = typeof sqliteSchema.oauthClients.$inferSelect
export type InsertOauthClientRow = typeof sqliteSchema.oauthClients.$inferInsert

export type OauthAuthRequestRow = typeof sqliteSchema.oauthAuthRequests.$inferSelect
export type InsertOauthAuthRequestRow = typeof sqliteSchema.oauthAuthRequests.$inferInsert

export type OauthConsentRow = typeof sqliteSchema.oauthConsents.$inferSelect
export type InsertOauthConsentRow = typeof sqliteSchema.oauthConsents.$inferInsert

export type OauthCodeRow = typeof sqliteSchema.oauthCodes.$inferSelect
export type InsertOauthCodeRow = typeof sqliteSchema.oauthCodes.$inferInsert
