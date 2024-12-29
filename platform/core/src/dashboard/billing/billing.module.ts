import { Module } from '@nestjs/common'

import { StripeModule } from '@/dashboard/billing/stripe/stripe.module'

@Module({
  imports: [StripeModule],
  exports: [StripeModule]
})
export class BillingModule {}
