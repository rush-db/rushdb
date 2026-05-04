import { Global, Module } from '@nestjs/common'
import { ConfigService } from '@nestjs/config'

import { toBoolean } from '@/common/utils/toBolean'
import { BillingClientModule } from '@/core/billing-client/billing-client.module'
import { BILLING_ACCOUNT_PORT } from '@/core/billing-policy/billing-account.port'
import { BILLING_POLICY_PORT } from '@/core/billing-policy/billing-policy.port'
import { CloudBillingAccountService } from '@/core/billing-policy/cloud-billing-account.service'
import { CloudBillingPolicyService } from '@/core/billing-policy/cloud-billing-policy.service'
import { SelfHostedBillingAccountService } from '@/core/billing-policy/self-hosted-billing-account.service'
import { SelfHostedBillingPolicyService } from '@/core/billing-policy/self-hosted-billing-policy.service'

@Global()
@Module({
  imports: [BillingClientModule],
  providers: [
    CloudBillingPolicyService,
    SelfHostedBillingPolicyService,
    CloudBillingAccountService,
    SelfHostedBillingAccountService,
    {
      provide: BILLING_POLICY_PORT,
      inject: [ConfigService, CloudBillingPolicyService, SelfHostedBillingPolicyService],
      useFactory: (
        configService: ConfigService,
        cloudPolicyService: CloudBillingPolicyService,
        selfHostedPolicyService: SelfHostedBillingPolicyService
      ) => {
        return toBoolean(configService.get('RUSHDB_SELF_HOSTED')) ? selfHostedPolicyService : (
            cloudPolicyService
          )
      }
    },
    {
      provide: BILLING_ACCOUNT_PORT,
      inject: [ConfigService, CloudBillingAccountService, SelfHostedBillingAccountService],
      useFactory: (
        configService: ConfigService,
        cloudAccountService: CloudBillingAccountService,
        selfHostedAccountService: SelfHostedBillingAccountService
      ) => {
        return toBoolean(configService.get('RUSHDB_SELF_HOSTED')) ? selfHostedAccountService : (
            cloudAccountService
          )
      }
    }
  ],
  exports: [BILLING_POLICY_PORT, BILLING_ACCOUNT_PORT]
})
export class BillingPolicyModule {}
