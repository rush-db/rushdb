import { HttpModule } from '@nestjs/axios'
import { Module } from '@nestjs/common'

import { BillingClientService } from './billing-client.service'

@Module({
  imports: [HttpModule],
  providers: [BillingClientService],
  exports: [BillingClientService]
})
export class BillingClientModule {}
