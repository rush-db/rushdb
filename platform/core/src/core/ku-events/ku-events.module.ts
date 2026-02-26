import { HttpModule } from '@nestjs/axios'
import { Global, Module } from '@nestjs/common'
import { ScheduleModule } from '@nestjs/schedule'

import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { StorageFootprintScheduler } from '@/core/ku-events/storage-footprint.scheduler'

/**
 * KuEventsModule
 *
 * Provides the KuEventsService globally so any module can inject it
 * without adding it to their own imports array. The Global decorator
 * ensures a single shared instance across the application.
 *
 * Also registers the StorageFootprintScheduler, which emits daily
 * STORAGE_FOOTPRINT KU events for every project that has stored records.
 */
@Global()
@Module({
  imports: [HttpModule, ScheduleModule.forRoot()],
  providers: [KuEventsService, StorageFootprintScheduler],
  exports: [KuEventsService]
})
export class KuEventsModule {}
