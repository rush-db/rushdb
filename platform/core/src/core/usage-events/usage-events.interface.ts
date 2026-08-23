import {
  UsageOperation,
  DeploymentKind,
  USAGE_EVENT_SCHEMA_VERSION,
  UsageEventOutcome,
  UsageEventSource
} from '@/core/usage-events/usage-events.constants'

/** Measured dimensions recorded with each usage event. */
export interface CostDimensions {
  scalarValuesWritten?: number
  valueBytesWritten?: number
  retainedValueBytes?: number
  relationshipCount?: number
  durationMs?: number
  dbDurationMs?: number
  resultRecords?: number
  recordsScanned?: number
  targetScopeRecords?: number
  traversalDepth?: number
  aggregation?: boolean
  vectorSearch?: boolean
  vectorDimensions?: number
  vectorsWritten?: number
  candidatesSearched?: number
  embeddingProvider?: string
  embeddingModel?: string
  embeddingInputTokens?: number
  llmModel?: string
  llmInputTokens?: number
  llmOutputTokens?: number
  generatedQueries?: number
  executedQueries?: number
  apiCpuMs?: number
  egressBytes?: number
  cacheStatus?: 'hit' | 'miss' | 'bypass'
  neo4jIndexBytes?: number
  storageBytes?: number
}

/**
 * Canonical usage event v3.
 *
 * Immutable once emitted. One external request produces exactly one event —
 * internal retries, count subqueries and tool-call fanout never create
 * additional events.
 *
 * Never include query text, record contents, credentials, connection strings,
 * personal data or arbitrary request bodies here.
 */
export interface UsageEventV3 {
  schemaVersion: typeof USAGE_EVENT_SCHEMA_VERSION
  eventId: string
  /** sha256(workspaceId + projectId + requestId + operationClass) — stable across transport retries. */
  idempotencyKey: string
  workspaceId: string
  projectId?: string
  occurredAt: string
  receivedAt?: string
  operationClass: UsageOperation
  routeOrTool: string
  source: UsageEventSource
  outcome: UsageEventOutcome
  costDimensions: CostDimensions
  classificationVersion: string
  deployment: DeploymentKind
  /** Strictly allowlisted metadata only. */
  metadata?: Record<string, unknown>
}
