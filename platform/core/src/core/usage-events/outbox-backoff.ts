import { OUTBOX_RETRY_BASE_MS, OUTBOX_RETRY_MAX_MS } from '@/core/usage-events/usage-events.constants'

/**
 * Exponential backoff with jitter for outbox redelivery.
 *
 * delay(attempt) = min(base * 2^(attempt-1), max) * (0.5 + jitter)
 * where jitter ∈ [0, 1). Pure and injectable for deterministic tests.
 */
export function computeBackoffMs(
  attemptCount: number,
  random: () => number = Math.random,
  baseMs: number = OUTBOX_RETRY_BASE_MS,
  maxMs: number = OUTBOX_RETRY_MAX_MS
): number {
  const attempt = Math.max(1, Math.floor(attemptCount))
  const exponential = Math.min(baseMs * Math.pow(2, attempt - 1), maxMs)
  return Math.floor(exponential * (0.5 + random()))
}
