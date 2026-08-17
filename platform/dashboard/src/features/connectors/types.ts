export type ConnectorType = 'postgres' | 'mysql' | 'mongodb'
export type ConnectorStatus = 'paused' | 'running' | 'error' | 'testing' | 'deleted'

/** Connector descriptor as served by the synx provider catalog (shape only). */
export type SynxConnectorDescriptor = {
  id: string
  type: string
  name: string
  description?: string
  version: string
  schemaVersion: string
  capabilities: {
    batchModes: string[]
    deletionModes: string[]
    relationEvidence: boolean
    oauth?: boolean
    webhooks?: boolean
  }
  entitlement?: 'free' | 'paid' | 'top_tier'
  /** Inline monochrome SVG icon, declared by the connector's spec. */
  icon?: string
  fields: {
    key: string
    label: string
    type: string
    required: boolean
    secret?: boolean
    description?: string
  }[]
}

export type SynxUnavailableConnector = {
  id: string
  name: string
  requiredTier: 'free' | 'paid' | 'top_tier'
  reason: string
  icon?: string
}

export type SynxConnectorCatalog = {
  connectors: SynxConnectorDescriptor[]
  unavailable: SynxUnavailableConnector[]
}

export type ConnectorTransform = {
  naming?: 'preserve' | 'camelCase'
  singularize?: boolean
  mergeStrategy?: 'append' | 'rewrite'
  fields?: {
    ignore?: string[]
    hoist?: Record<string, string>
    keyPath?: string
  }
  labels?: Record<string, string>
  entities?: string[]
}

export type ConnectorHealth = {
  score: number
  level: 'healthy' | 'degraded' | 'critical'
  reasons: string[]
}

export type Connector = {
  id: string
  projectId: string
  name: string
  type: ConnectorType
  config: Record<string, unknown>
  transform: ConnectorTransform
  status: ConnectorStatus
  lastError?: string | null
  lagMs?: number | null
  stats?: Record<string, unknown>
  health?: ConnectorHealth
  secrets: Record<string, string>
  createdAt: string
  updatedAt: string
}

export type ConnectorEvent = {
  id: string
  connectorId: string
  level: 'debug' | 'info' | 'warn' | 'error' | string
  type: string
  message: string
  metadata?: Record<string, unknown> | string | null
  createdAt: string
}

export type ConnectorCommand = {
  id: string
  connectorId: string
  type: 'test' | 'discover' | 'databases'
  status: 'pending' | 'claimed' | 'completed' | 'failed'
  result?: string | null
  errorMessage?: string | null
  createdAt: string
  completedAt?: string | null
}

export type ConnectorRun = {
  id: string
  connectorId: string
  projectId: string
  workerId: string
  trigger: string
  status: 'running' | 'stopped' | 'failed'
  phase: string
  recordsRead: number
  recordsWritten: number
  recordsRejected: number
  errorMessage?: string | null
  startedAt: string
  completedAt?: string | null
  heartbeatAt: string
}

export type ConnectorRejection = {
  id: string
  connectorId: string
  projectId: string
  batchId: string
  operationIndex?: number | null
  sourceIdHash?: string | null
  code: string
  message: string
  retryable: number
  occurrenceCount: number
  firstSeenAt: string
  lastSeenAt: string
  resolved: number
  createdAt: string
}

export type CreateConnectorInput = {
  name: string
  /** Database types plus any worker-registered spec id (never hardcoded). */
  type: string
  config: Record<string, unknown>
  secrets?: Record<string, unknown>
  transform?: ConnectorTransform
}
