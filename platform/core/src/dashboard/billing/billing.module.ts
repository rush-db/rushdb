import { forwardRef, Module } from '@nestjs/common'

import { BillingClientModule } from '@/core/billing-client/billing-client.module'
import { BillingController } from '@/dashboard/billing/billing.controller'
import { WorkspaceModule } from '@/dashboard/workspace/workspace.module'

@Module({
  imports: [BillingClientModule, forwardRef(() => WorkspaceModule)],
  exports: [BillingClientModule],
  controllers: [BillingController]
})
export class BillingModule {}
