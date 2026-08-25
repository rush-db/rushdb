import { bigint, boolean, integer, pgTable, primaryKey, text, uniqueIndex } from 'drizzle-orm/pg-core'

export const users = pgTable('users', {
  id: text('id').primaryKey(),
  login: text('login').notNull().unique(),
  isEmail: boolean('is_email').notNull().default(false),
  firstName: text('first_name'),
  lastName: text('last_name'),
  confirmed: boolean('confirmed').notNull().default(false),
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

export const workspaces = pgTable('workspaces', {
  id: text('id').primaryKey(),
  name: text('name').notNull(),
  created: text('created').notNull(),
  edited: text('edited'),
  stats: text('stats')
})

export const workspaceMembers = pgTable(
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

export const workspaceInvites = pgTable('workspace_invites', {
  id: text('id').primaryKey(),
  workspaceId: text('workspace_id')
    .notNull()
    .references(() => workspaces.id, { onDelete: 'cascade' }),
  email: text('email').notNull(),
  createdAt: text('created_at').notNull()
})

export const workspaceIdentityProviders = pgTable(
  'workspace_identity_providers',
  {
    id: text('id').primaryKey(),
    workspaceId: text('workspace_id')
      .notNull()
      .references(() => workspaces.id, { onDelete: 'cascade' }),
    type: text('type').notNull(), // 'saml' | 'oidc'
    enabled: boolean('enabled').notNull().default(false),
    enforced: boolean('enforced').notNull().default(false),
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

export const projects = pgTable('projects', {
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

export const projectAccess = pgTable(
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

export const tokens = pgTable('tokens', {
  id: text('id').primaryKey(),
  projectId: text('project_id')
    .notNull()
    .references(() => projects.id, { onDelete: 'cascade' }),
  name: text('name').notNull(),
  expiration: bigint('expiration', { mode: 'number' }).notNull(),
  created: text('created').notNull(),
  description: text('description'),
  value: text('value').notNull(),
  prefixValue: text('prefix_value'),
  consentId: text('consent_id'),
  level: text('level').notNull().default('write')
})

export const oauthClients = pgTable(
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

export const oauthAuthRequests = pgTable('oauth_auth_requests', {
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

export const oauthConsents = pgTable('oauth_consents', {
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

export const oauthCodes = pgTable('oauth_codes', {
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

export const oauthRefreshTokens = pgTable('oauth_refresh_tokens', {
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

export const embeddingIndexes = pgTable(
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
    enabled: boolean('enabled').notNull().default(true),
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

export const relationshipPatterns = pgTable(
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

export const relationshipAnalysisQueue = pgTable('relationship_analysis_queue', {
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

export const connectors = pgTable('connectors', {
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

export const connectorSecrets = pgTable('connector_secrets', {
  connectorId: text('connector_id')
    .primaryKey()
    .references(() => connectors.id, { onDelete: 'cascade' }),
  provider: text('provider').notNull().default('local'),
  secretRef: text('secret_ref'),
  ciphertext: text('ciphertext'),
  createdAt: text('created_at').notNull(),
  updatedAt: text('updated_at').notNull()
})

export const connectorOffsets = pgTable(
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

export const connectorEvents = pgTable('connector_events', {
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

export const connectorLeases = pgTable('connector_leases', {
  connectorId: text('connector_id')
    .primaryKey()
    .references(() => connectors.id, { onDelete: 'cascade' }),
  workerId: text('worker_id').notNull(),
  leaseUntil: text('lease_until').notNull(),
  heartbeatAt: text('heartbeat_at').notNull(),
  createdAt: text('created_at').notNull(),
  updatedAt: text('updated_at').notNull()
})

export const savedQueries = pgTable('saved_queries', {
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

// JSON snapshots in text columns: link_spec, parse_options, import_options, checkpoint, metadata
export const importRuns = pgTable(
  'import_runs',
  {
    id: text('id').primaryKey(),
    projectId: text('project_id')
      .notNull()
      .references(() => projects.id, { onDelete: 'cascade' }),
    workspaceId: text('workspace_id'),
    createdByType: text('created_by_type').notNull().default('user'),
    createdById: text('created_by_id'),
    name: text('name'),
    idempotencyKeyHash: text('idempotency_key_hash'),
    status: text('status').notNull().default('draft'),
    failurePolicy: text('failure_policy').notNull().default('continue'),
    manifestVersion: integer('manifest_version').notNull().default(0),
    totalFiles: integer('total_files').notNull().default(0),
    totalBytes: bigint('total_bytes', { mode: 'number' }).notNull().default(0),
    uploadedBytes: bigint('uploaded_bytes', { mode: 'number' }).notNull().default(0),
    parsedUnits: integer('parsed_units').notNull().default(0),
    recordsCommitted: integer('records_committed').notNull().default(0),
    relationshipsCommitted: integer('relationships_committed').notNull().default(0),
    skippedUnits: integer('skipped_units').notNull().default(0),
    failedFiles: integer('failed_files').notNull().default(0),
    cancelRequestedAt: text('cancel_requested_at'),
    startedAt: text('started_at'),
    finalizedAt: text('finalized_at'),
    retentionUntil: text('retention_until'),
    createdAt: text('created_at').notNull(),
    updatedAt: text('updated_at').notNull()
  },
  (t) => [uniqueIndex('import_run_idempotency_uniq').on(t.projectId, t.idempotencyKeyHash)]
)

export const importRunFiles = pgTable(
  'import_run_files',
  {
    id: text('id').primaryKey(),
    runId: text('run_id')
      .notNull()
      .references(() => importRuns.id, { onDelete: 'cascade' }),
    projectId: text('project_id')
      .notNull()
      .references(() => projects.id, { onDelete: 'cascade' }),
    workspaceId: text('workspace_id'),
    ordinal: integer('ordinal').notNull(),
    clientFileId: text('client_file_id').notNull(),
    fileName: text('file_name').notNull(),
    declaredSizeBytes: bigint('declared_size_bytes', { mode: 'number' }).notNull().default(0),
    format: text('format').notNull(),
    jsonShape: text('json_shape'),
    role: text('role').notNull().default('records'),
    rootLabel: text('root_label'),
    linkSpec: text('link_spec'),
    parseOptions: text('parse_options'),
    importOptions: text('import_options'),
    sourceGeneration: integer('source_generation').notNull().default(1),
    storageProvider: text('storage_provider').notNull().default('memory'),
    storageKey: text('storage_key'),
    storageUploadId: text('storage_upload_id'),
    objectSizeBytes: bigint('object_size_bytes', { mode: 'number' }),
    checksumAlgorithm: text('checksum_algorithm'),
    checksumValue: text('checksum_value'),
    status: text('status').notNull().default('awaiting_upload'),
    stage: text('stage').notNull().default('upload'),
    processedBytes: bigint('processed_bytes', { mode: 'number' }).notNull().default(0),
    parsedUnits: integer('parsed_units').notNull().default(0),
    committedUnits: integer('committed_units').notNull().default(0),
    recordsCommitted: integer('records_committed').notNull().default(0),
    relationshipsCommitted: integer('relationships_committed').notNull().default(0),
    linksResolved: integer('links_resolved').notNull().default(0),
    linksUnresolved: integer('links_unresolved').notNull().default(0),
    skippedUnits: integer('skipped_units').notNull().default(0),
    currentBatch: integer('current_batch').notNull().default(0),
    checkpoint: text('checkpoint'),
    attemptCount: integer('attempt_count').notNull().default(0),
    maxAttempts: integer('max_attempts').notNull().default(3),
    notBefore: text('not_before'),
    leaseOwner: text('lease_owner'),
    leaseGeneration: integer('lease_generation').notNull().default(0),
    leaseUntil: text('lease_until'),
    heartbeatAt: text('heartbeat_at'),
    cancelRequestedAt: text('cancel_requested_at'),
    lastErrorCode: text('last_error_code'),
    lastErrorMessage: text('last_error_message'),
    startedAt: text('started_at'),
    finishedAt: text('finished_at'),
    createdAt: text('created_at').notNull(),
    updatedAt: text('updated_at').notNull()
  },
  (t) => [
    uniqueIndex('import_file_client_id_uniq').on(t.runId, t.clientFileId),
    uniqueIndex('import_file_ordinal_uniq').on(t.runId, t.ordinal)
  ]
)

export const importRunEvents = pgTable('import_run_events', {
  id: text('id').primaryKey(),
  runId: text('run_id')
    .notNull()
    .references(() => importRuns.id, { onDelete: 'cascade' }),
  fileId: text('file_id'),
  projectId: text('project_id')
    .notNull()
    .references(() => projects.id, { onDelete: 'cascade' }),
  type: text('type').notNull(),
  fromStatus: text('from_status'),
  toStatus: text('to_status'),
  code: text('code'),
  message: text('message'),
  attempt: integer('attempt'),
  leaseGeneration: integer('lease_generation'),
  metadata: text('metadata'),
  createdAt: text('created_at').notNull()
})

export const importErrorSamples = pgTable('import_error_samples', {
  id: text('id').primaryKey(),
  runId: text('run_id')
    .notNull()
    .references(() => importRuns.id, { onDelete: 'cascade' }),
  fileId: text('file_id')
    .notNull()
    .references(() => importRunFiles.id, { onDelete: 'cascade' }),
  projectId: text('project_id')
    .notNull()
    .references(() => projects.id, { onDelete: 'cascade' }),
  sourceUnit: integer('source_unit'),
  lineNumber: integer('line_number'),
  columnNumber: integer('column_number'),
  code: text('code').notNull(),
  message: text('message'),
  createdAt: text('created_at').notNull()
})

export const pgSchema = {
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
  savedQueries,
  importRuns,
  importRunFiles,
  importRunEvents,
  importErrorSamples
}

export type PgSchema = typeof pgSchema
