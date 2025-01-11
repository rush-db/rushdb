import { Module } from '@nestjs/common'

import { CoreModule } from '@/core/core.module'
import { DashboardModule } from '@/dashboard/dashboard.module'

import { BackupService } from './backup.service'

@Module({
  imports: [CoreModule, DashboardModule],
  exports: [BackupService],
  providers: [BackupService]
})
export class BackupModule {}
