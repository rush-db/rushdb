/** Usage events v3 — telemetry contract constants. */

export const USAGE_EVENT_SCHEMA_VERSION = 3

/** Version of the telemetry measurement schema. */
export const MEASUREMENT_VERSION = '2026-08-20.1'

/** Operation classes a usage event can describe. */
export const USAGE_OPERATIONS = [
  'standard_query',
  'advanced_query',
  'deep_analysis',
  'nl_planning',
  'schema_discovery',
  'write',
  'ingestion',
  'embedding',
  'context_snapshot',
  'connector_run',
  'export'
] as const

export type UsageOperation = (typeof USAGE_OPERATIONS)[number]

/** Where the request originated. */
export const USAGE_EVENT_SOURCES = ['rest', 'sdk', 'mcp', 'dashboard', 'connector', 'scheduler'] as const

export type UsageEventSource = (typeof USAGE_EVENT_SOURCES)[number]

/** Outcome of the observed operation. */
export const USAGE_EVENT_OUTCOMES = ['succeeded', 'failed', 'rejected', 'cancelled'] as const

export type UsageEventOutcome = (typeof USAGE_EVENT_OUTCOMES)[number]

/** Deployment kind the event was produced in. */
export const DEPLOYMENT_KINDS = ['managed', 'byoc', 'dedicated', 'self-hosted'] as const

export type DeploymentKind = (typeof DEPLOYMENT_KINDS)[number]

/** Outbox row states for durable event delivery. */
export const OUTBOX_STATUSES = ['pending', 'delivered', 'dead'] as const

export type OutboxStatus = (typeof OUTBOX_STATUSES)[number]

export const OUTBOX_MAX_ATTEMPTS = 10

/** Base delay (ms) for the first delivery retry; grows exponentially. */
export const OUTBOX_RETRY_BASE_MS = 1_000

/** Upper bound for a single retry delay. */
export const OUTBOX_RETRY_MAX_MS = 15 * 60_000

/** Default number of outbox rows claimed per delivery batch. */
export const OUTBOX_BATCH_SIZE = 100
