import { computeBackoffMs } from '@/core/usage-events/outbox-backoff'
import {
  OUTBOX_MAX_ATTEMPTS,
  OUTBOX_RETRY_BASE_MS,
  OUTBOX_RETRY_MAX_MS
} from '@/core/usage-events/usage-events.constants'

describe('outbox-backoff', () => {
  it('doubles the delay exponentially per attempt', () => {
    const noJitter = () => 0.5 // jitter factor becomes exactly 1.0
    expect(computeBackoffMs(1, noJitter)).toBe(OUTBOX_RETRY_BASE_MS)
    expect(computeBackoffMs(2, noJitter)).toBe(OUTBOX_RETRY_BASE_MS * 2)
    expect(computeBackoffMs(3, noJitter)).toBe(OUTBOX_RETRY_BASE_MS * 4)
  })

  it('caps the delay at the configured maximum', () => {
    const noJitter = () => 0.5
    expect(computeBackoffMs(30, noJitter)).toBe(OUTBOX_RETRY_MAX_MS)
  })

  it('applies ±50% jitter around the exponential delay', () => {
    const low = computeBackoffMs(2, () => 0) // factor 0.5×
    const high = computeBackoffMs(2, () => 0.999999) // factor ~1.5×

    expect(low).toBeGreaterThanOrEqual(OUTBOX_RETRY_BASE_MS)
    expect(low).toBeLessThan(OUTBOX_RETRY_BASE_MS * 2)
    expect(high).toBeGreaterThan(OUTBOX_RETRY_BASE_MS * 2)
    expect(high).toBeLessThanOrEqual(OUTBOX_RETRY_BASE_MS * 3)
  })

  it('treats non-positive attempt counts as the first attempt', () => {
    const noJitter = () => 0.5
    expect(computeBackoffMs(0, noJitter)).toBe(OUTBOX_RETRY_BASE_MS)
    expect(computeBackoffMs(-5, noJitter)).toBe(OUTBOX_RETRY_BASE_MS)
  })

  it('keeps attempts within dead-letter bounds meaningfully spaced', () => {
    const noJitter = () => 0.5
    const totalNoJitter = Array.from({ length: OUTBOX_MAX_ATTEMPTS - 1 }, (_, i) =>
      computeBackoffMs(i + 1, noJitter)
    ).reduce((a, b) => a + b, 0)

    // Bounded worst-case wait before dead-lettering (~< 4h with current constants).
    expect(totalNoJitter).toBeLessThan(4 * 60 * 60 * 1000)
  })
})
