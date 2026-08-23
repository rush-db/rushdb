import { Injectable, Logger } from '@nestjs/common'
import { Interval } from '@nestjs/schedule'

import { UsageEventsService } from '@/core/usage-events/usage-events.service'

const FLUSH_INTERVAL_MS = 30_000

/**
 * Periodically drains the usage-event outbox to the configured delivery
 * endpoint.
 *
 * Delivery is best-effort by design: failures are retried on the next tick
 * (and with exponential backoff per row), so delivery-target downtime never
 * affects application traffic.
 */
@Injectable()
export class UsageEventOutboxScheduler {
  private readonly logger = new Logger(UsageEventOutboxScheduler.name)
  private draining = false

  constructor(private readonly usageEventsService: UsageEventsService) {}

  @Interval(FLUSH_INTERVAL_MS)
  async flush(): Promise<void> {
    if (this.draining || !this.usageEventsService.isEnabled()) {
      return
    }

    this.draining = true
    try {
      // Drain in bounded rounds so a large backlog catches up steadily
      // without monopolizing the event loop.
      for (let round = 0; round < 10; round++) {
        const delivered = await this.usageEventsService.flushDueEvents()
        if (delivered === 0) {
          break
        }
      }
    } catch (error) {
      this.logger.warn(`Usage outbox flush failed: ${error?.message}`)
    } finally {
      this.draining = false
    }
  }
}
