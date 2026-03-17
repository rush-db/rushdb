import { Module } from '@nestjs/common'

import { BillingClientModule } from '@/core/billing-client/billing-client.module'
import { BillingController } from '@/dashboard/billing/billing.controller'

@Module({
  imports: [BillingClientModule],
  exports: [BillingClientModule],
  controllers: [BillingController]
})
export class BillingModule {}
