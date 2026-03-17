import { Module } from '@nestjs/common'
import { HttpModule } from '@nestjs/axios'
import { BillingClientService } from './billing-client.service'

@Module({
  imports: [HttpModule],
  providers: [BillingClientService],
  exports: [BillingClientService]
})
export class BillingClientModule {}
