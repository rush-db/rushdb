export const CONNECTOR_TYPES = ['postgres', 'mongodb'] as const
export type ConnectorType = (typeof CONNECTOR_TYPES)[number]

export const CONNECTOR_STATUSES = ['paused', 'running', 'error', 'testing', 'deleted'] as const
export type ConnectorStatus = (typeof CONNECTOR_STATUSES)[number]

export type ConnectorTransform = {
  labels?: Record<string, string>
  singularize?: boolean
  entities?: string[]
  naming?: 'preserve' | 'camelCase'
  flattenSeparator?: string
  syncKeyField?: string
  syncedAtField?: string
  mergeStrategy?: 'append' | 'rewrite'
  fields?: {
    ignore?: string[]
    hoist?: Record<string, string>
    keyPath?: string
  }
}

export type ConnectorPublicConfig =
  | {
      host: string
      port?: number
      database: string
      user?: string
      ssl?: boolean
      tables?: string[]
      snapshot?: boolean
      slot?: string
    }
  | {
      host?: string
      uri?: string
      database: string
      collections?: string[]
      snapshot?: boolean
    }

export type ConnectorSecretInput = {
  password?: string
  uri?: string
  connString?: string
}

export type ConnectorStats = {
  eventsIn?: number
  batchesApplied?: number
  opsApplied?: number
  eventsSkipped?: number
  batchesFailed?: number
  lastEventUnixMs?: number
}
