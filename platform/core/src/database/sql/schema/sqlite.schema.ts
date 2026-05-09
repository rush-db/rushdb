import { integer, primaryKey, sqliteTable, text, uniqueIndex } from 'drizzle-orm/sqlite-core'

export const users = sqliteTable('users', {
  id: text('id').primaryKey(),
  login: text('login').notNull().unique(),
  isEmail: integer('is_email', { mode: 'boolean' }).notNull().default(false),
  firstName: text('first_name'),
  lastName: text('last_name'),
  confirmed: integer('confirmed', { mode: 'boolean' }).notNull().default(false),
  status: text('status'),
  created: text('created').notNull(),
  edited: text('edited'),
  lastActivity: text('last_activity'),
  googleAuth: text('google_auth'),
  githubAuth: text('github_auth'),
  password: text('password'),
  deletedDate: text('deleted_date'),
  settings: text('settings')
})

export const workspaces = sqliteTable('workspaces', {
  id: text('id').primaryKey(),
  name: text('name').notNull(),
  created: text('created').notNull(),
  edited: text('edited'),
  stats: text('stats')
})

export const workspaceMembers = sqliteTable(
  'workspace_members',
  {
    workspaceId: text('workspace_id')
      .notNull()
      .references(() => workspaces.id, { onDelete: 'cascade' }),
    userId: text('user_id')
      .notNull()
      .references(() => users.id, { onDelete: 'cascade' }),
    role: text('role').notNull(),
    since: text('since').notNull()
  },
  (t) => [primaryKey({ columns: [t.workspaceId, t.userId] })]
)

export const workspaceInvites = sqliteTable('workspace_invites', {
  id: text('id').primaryKey(),
  workspaceId: text('workspace_id')
    .notNull()
    .references(() => workspaces.id, { onDelete: 'cascade' }),
  email: text('email').notNull(),
  createdAt: text('created_at').notNull()
})

export const projects = sqliteTable('projects', {
  id: text('id').primaryKey(),
  workspaceId: text('workspace_id')
    .notNull()
    .references(() => workspaces.id, { onDelete: 'cascade' }),
  name: text('name').notNull(),
  description: text('description'),
  created: text('created').notNull(),
  edited: text('edited'),
  deleted: text('deleted'),
  status: text('status'),
  stats: text('stats'),
  customDb: text('custom_db'),
  ontologyCache: text('ontology_cache'),
  ontologyCachedAt: text('ontology_cached_at')
})

export const projectAccess = sqliteTable(
  'project_access',
  {
    projectId: text('project_id')
      .notNull()
      .references(() => projects.id, { onDelete: 'cascade' }),
    userId: text('user_id')
      .notNull()
      .references(() => users.id, { onDelete: 'cascade' }),
    role: text('role').notNull(),
    since: text('since').notNull()
  },
  (t) => [primaryKey({ columns: [t.projectId, t.userId] })]
)

export const tokens = sqliteTable('tokens', {
  id: text('id').primaryKey(),
  projectId: text('project_id')
    .notNull()
    .references(() => projects.id, { onDelete: 'cascade' }),
  name: text('name').notNull(),
  expiration: integer('expiration').notNull(),
  created: text('created').notNull(),
  description: text('description'),
  value: text('value').notNull(),
  prefixValue: text('prefix_value'),
  consentId: text('consent_id'),
  level: text('level').notNull().default('write')
})

export const oauthClients = sqliteTable(
  'oauth_clients',
  {
    id: text('id').primaryKey(),
    clientName: text('client_name').notNull(),
    redirectUris: text('redirect_uris').notNull(),
    tokenEndpointAuthMethod: text('token_endpoint_auth_method'),
    applicationType: text('application_type'),
    created: text('created').notNull()
  },
  (t) => [uniqueIndex('uq_oauth_client_name_uris').on(t.clientName, t.redirectUris)]
)

export const oauthAuthRequests = sqliteTable('oauth_auth_requests', {
  id: text('id').primaryKey(),
  clientId: text('client_id')
    .notNull()
    .references(() => oauthClients.id, { onDelete: 'cascade' }),
  redirectUri: text('redirect_uri').notNull(),
  scope: text('scope'),
  resource: text('resource'),
  codeChallenge: text('code_challenge').notNull(),
  codeChallengeMethod: text('code_challenge_method').notNull(),
  state: text('state'),
  created: text('created').notNull(),
  expiresAt: text('expires_at').notNull()
})

export const oauthConsents = sqliteTable('oauth_consents', {
  id: text('id').primaryKey(),
  userId: text('user_id').notNull(),
  clientId: text('client_id')
    .notNull()
    .references(() => oauthClients.id, { onDelete: 'cascade' }),
  projectId: text('project_id').notNull(),
  resource: text('resource'),
  scope: text('scope').notNull(),
  created: text('created').notNull(),
  revokedAt: text('revoked_at')
})

export const oauthCodes = sqliteTable('oauth_codes', {
  id: text('id').primaryKey(),
  consentId: text('consent_id')
    .notNull()
    .references(() => oauthConsents.id, { onDelete: 'cascade' }),
  clientId: text('client_id').notNull(),
  redirectUri: text('redirect_uri').notNull(),
  resource: text('resource'),
  scope: text('scope'),
  codeChallenge: text('code_challenge').notNull(),
  codeChallengeMethod: text('code_challenge_method').notNull(),
  created: text('created').notNull(),
  expiresAt: text('expires_at').notNull()
})

export const embeddingIndexes = sqliteTable(
  'embedding_indexes',
  {
    id: text('id').primaryKey(),
    projectId: text('project_id')
      .notNull()
      .references(() => projects.id, { onDelete: 'cascade' }),
    label: text('label').notNull().default(''),
    propertyName: text('property_name').notNull(),
    modelKey: text('model_key').notNull(),
    sourceType: text('source_type').notNull().default('managed'),
    similarityFunction: text('similarity_function').notNull().default('cosine'),
    dimensions: integer('dimensions').notNull(),
    vectorPropertyName: text('vector_property_name').notNull(),
    enabled: integer('enabled', { mode: 'boolean' }).notNull().default(true),
    status: text('status').notNull().default('pending'),
    createdAt: text('created_at').notNull(),
    updatedAt: text('updated_at').notNull()
  },
  (t) => [
    uniqueIndex('emb_idx_signature_uniq').on(
      t.projectId,
      t.propertyName,
      t.label,
      t.sourceType,
      t.similarityFunction,
      t.dimensions
    )
  ]
)

export const sqliteSchema = {
  users,
  workspaces,
  workspaceMembers,
  workspaceInvites,
  projects,
  projectAccess,
  tokens,
  oauthClients,
  oauthAuthRequests,
  oauthConsents,
  oauthCodes,
  embeddingIndexes
}

export type SqliteSchema = typeof sqliteSchema
