export type ConnectorType = 'postgres' | 'mongodb'
export type ConnectorStatus = 'paused' | 'running' | 'error' | 'testing' | 'deleted'

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

export type CreateConnectorInput = {
  name: string
  type: ConnectorType
  config: Record<string, unknown>
  secrets?: Record<string, unknown>
  transform?: ConnectorTransform
}
