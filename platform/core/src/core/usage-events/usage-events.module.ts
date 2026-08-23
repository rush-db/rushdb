import { HttpModule } from '@nestjs/axios'
import { Global, Module } from '@nestjs/common'
import { ScheduleModule } from '@nestjs/schedule'

import { UsageEventOutboxScheduler } from '@/core/usage-events/usage-event-outbox.scheduler'
import { UsageEventOutboxService } from '@/core/usage-events/usage-event-outbox.service'
import { UsageEventsService } from '@/core/usage-events/usage-events.service'

/**
 * UsageEventsModule
 *
 * Provides usage events v3 production globally: telemetry measurement and a
 * durable SQL outbox with scheduled delivery to the configured endpoint.
 *
 * Coexists with KuEventsModule.
 */
@Global()
@Module({
  imports: [HttpModule, ScheduleModule],
  providers: [UsageEventOutboxService, UsageEventsService, UsageEventOutboxScheduler],
  exports: [UsageEventsService, UsageEventOutboxService]
})
export class UsageEventsModule {}
