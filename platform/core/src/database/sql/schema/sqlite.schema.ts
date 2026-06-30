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
  samlAuth: text('saml_auth'),
  oidcAuth: text('oidc_auth'),
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

export const workspaceIdentityProviders = sqliteTable(
  'workspace_identity_providers',
  {
    id: text('id').primaryKey(),
    workspaceId: text('workspace_id')
      .notNull()
      .references(() => workspaces.id, { onDelete: 'cascade' }),
    type: text('type').notNull(), // 'saml' | 'oidc'
    enabled: integer('enabled', { mode: 'boolean' }).notNull().default(false),
    enforced: integer('enforced', { mode: 'boolean' }).notNull().default(false),
    domains: text('domains').notNull().default('[]'), // JSON string[]
    defaultRole: text('default_role').notNull().default('developer'),
    groupMappings: text('group_mappings'), // JSON { [idpGroup]: role }
    // SAML
    samlEntityId: text('saml_entity_id'),
    samlSsoUrl: text('saml_sso_url'),
    samlCertificate: text('saml_certificate'),
    // OIDC
    oidcIssuer: text('oidc_issuer'),
    oidcClientId: text('oidc_client_id'),
    oidcClientSecretEncrypted: text('oidc_client_secret_encrypted'),
    createdAt: text('created_at').notNull(),
    updatedAt: text('updated_at').notNull()
  },
  (t) => [uniqueIndex('workspace_idp_workspace_type_uniq').on(t.workspaceId, t.type)]
)

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
  schemaCache: text('schema_cache'),
  schemaCachedAt: text('schema_cached_at')
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

/**
 * Stores hashed refresh token values (SHA-256 hex).
 * The raw token is returned to the client once and never persisted.
 * Deleting a consent row cascades to its refresh tokens.
 */
export const oauthRefreshTokens = sqliteTable('oauth_refresh_tokens', {
  /** SHA-256 hex digest of the raw token value */
  id: text('id').primaryKey(),
  consentId: text('consent_id')
    .notNull()
    .references(() => oauthConsents.id, { onDelete: 'cascade' }),
  clientId: text('client_id').notNull(),
  userId: text('user_id').notNull(),
  projectId: text('project_id').notNull(),
  scope: text('scope').notNull(),
  createdAt: text('created_at').notNull(),
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

export const relationshipPatterns = sqliteTable(
  'relationship_patterns',
  {
    id: text('id').primaryKey(),
    projectId: text('project_id')
      .notNull()
      .references(() => projects.id, { onDelete: 'cascade' }),
    sourceLabel: text('source_label').notNull(),
    sourceKey: text('source_key'),
    sourceWhere: text('source_where'),
    targetLabel: text('target_label').notNull(),
    targetKey: text('target_key'),
    targetWhere: text('target_where'),
    direction: text('direction').notNull().default('out'),
    type: text('type').notNull(),
    confidence: integer('confidence').notNull().default(0),
    status: text('status').notNull().default('suggested'),
    origin: text('origin').notNull().default('llm'),
    mode: text('mode').notNull().default('join_pattern'),
    signatureHash: text('signature_hash').notNull(),
    rationale: text('rationale'),
    sampleMatchCount: integer('sample_match_count'),
    lastAppliedAt: text('last_applied_at'),
    lastAnalyzedAt: text('last_analyzed_at'),
    lastError: text('last_error'),
    createdAt: text('created_at').notNull(),
    updatedAt: text('updated_at').notNull()
  },
  (t) => [uniqueIndex('rel_pattern_signature_uniq').on(t.projectId, t.signatureHash)]
)

export const relationshipAnalysisQueue = sqliteTable('relationship_analysis_queue', {
  projectId: text('project_id')
    .primaryKey()
    .references(() => projects.id, { onDelete: 'cascade' }),
  requestedAt: text('requested_at').notNull(),
  notBefore: text('not_before').notNull(),
  status: text('status').notNull().default('pending'),
  lastRunAt: text('last_run_at'),
  lastError: text('last_error'),
  createdAt: text('created_at').notNull(),
  updatedAt: text('updated_at').notNull()
})

export const connectors = sqliteTable('connectors', {
  id: text('id').primaryKey(),
  projectId: text('project_id')
    .notNull()
    .references(() => projects.id, { onDelete: 'cascade' }),
  name: text('name').notNull(),
  type: text('type').notNull(),
  config: text('config').notNull(),
  transform: text('transform').notNull(),
  status: text('status').notNull().default('paused'),
  lastError: text('last_error'),
  lagMs: integer('lag_ms'),
  stats: text('stats'),
  createdBy: text('created_by'),
  createdAt: text('created_at').notNull(),
  updatedAt: text('updated_at').notNull()
})

export const connectorSecrets = sqliteTable('connector_secrets', {
  connectorId: text('connector_id')
    .primaryKey()
    .references(() => connectors.id, { onDelete: 'cascade' }),
  provider: text('provider').notNull().default('local'),
  secretRef: text('secret_ref'),
  ciphertext: text('ciphertext'),
  createdAt: text('created_at').notNull(),
  updatedAt: text('updated_at').notNull()
})

export const connectorOffsets = sqliteTable(
  'connector_offsets',
  {
    connectorId: text('connector_id')
      .notNull()
      .references(() => connectors.id, { onDelete: 'cascade' }),
    partition: text('partition').notNull(),
    position: text('position').notNull(),
    updatedAt: text('updated_at').notNull()
  },
  (t) => [primaryKey({ columns: [t.connectorId, t.partition] })]
)

export const connectorEvents = sqliteTable('connector_events', {
  id: text('id').primaryKey(),
  connectorId: text('connector_id')
    .notNull()
    .references(() => connectors.id, { onDelete: 'cascade' }),
  projectId: text('project_id')
    .notNull()
    .references(() => projects.id, { onDelete: 'cascade' }),
  level: text('level').notNull().default('info'),
  type: text('type').notNull(),
  message: text('message').notNull(),
  metadata: text('metadata'),
  createdAt: text('created_at').notNull()
})

export const connectorLeases = sqliteTable('connector_leases', {
  connectorId: text('connector_id')
    .primaryKey()
    .references(() => connectors.id, { onDelete: 'cascade' }),
  workerId: text('worker_id').notNull(),
  leaseUntil: text('lease_until').notNull(),
  heartbeatAt: text('heartbeat_at').notNull(),
  createdAt: text('created_at').notNull(),
  updatedAt: text('updated_at').notNull()
})

export const savedQueries = sqliteTable('saved_queries', {
  id: text('id').primaryKey(),
  projectId: text('project_id')
    .notNull()
    .references(() => projects.id, { onDelete: 'cascade' }),
  name: text('name').notNull(),
  searchMode: text('search_mode').notNull().default('manual'),
  prompt: text('prompt'),
  searchQuery: text('search_query').notNull(),
  semanticIndexId: text('semantic_index_id'),
  createdBy: text('created_by'),
  createdAt: text('created_at').notNull(),
  updatedAt: text('updated_at').notNull()
})

export const sqliteSchema = {
  users,
  workspaces,
  workspaceMembers,
  workspaceInvites,
  workspaceIdentityProviders,
  projects,
  projectAccess,
  tokens,
  oauthClients,
  oauthAuthRequests,
  oauthConsents,
  oauthCodes,
  oauthRefreshTokens,
  embeddingIndexes,
  relationshipPatterns,
  relationshipAnalysisQueue,
  connectors,
  connectorSecrets,
  connectorOffsets,
  connectorEvents,
  connectorLeases,
  savedQueries
}

export type SqliteSchema = typeof sqliteSchema
