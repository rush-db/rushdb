import { HttpException, HttpStatus, Injectable } from '@nestjs/common'

import { BillingClientService } from '@/core/billing-client/billing-client.service'
import { BillingLimitOptions, BillingPolicyPort } from '@/core/billing-policy/billing-policy.port'

@Injectable()
export class CloudBillingPolicyService implements BillingPolicyPort {
  constructor(private readonly billingClientService: BillingClientService) {}

  async checkLimits(workspaceId: string, options?: BillingLimitOptions) {
    return this.billingClientService.checkLimits(workspaceId, options)
  }

  async isPlanActive(workspaceId: string): Promise<boolean> {
    const customer = await this.billingClientService.getCustomer(workspaceId)

    if (!customer) {
      return false
    }

    if (customer.subscriptionStatus === 'active' && customer.subscriptionId) {
      return true
    }

    return customer.plan !== 'free'
  }

  async isSsoAllowed(workspaceId: string): Promise<boolean> {
    const customer = await this.billingClientService.getCustomer(workspaceId)
    return customer?.plan === 'scale' || customer?.plan === 'enterprise'
  }

  async assertSsoAllowed(workspaceId: string): Promise<void> {
    if (!(await this.isSsoAllowed(workspaceId))) {
      throw new HttpException('SSO requires the Scale plan or higher.', HttpStatus.FORBIDDEN)
    }
  }

  async assertProjectOperationAllowed(workspaceId: string, options: BillingLimitOptions): Promise<void> {
    const check = await this.checkLimits(workspaceId, options)
    if (!check.allowed) {
      throw new HttpException(
        check.reason || 'Knowledge Unit (KU) limit exceeded. Upgrade your plan to continue.',
        HttpStatus.PAYMENT_REQUIRED
      )
    }
  }

  async assertEmbeddingIndexCreationAllowed(workspaceId: string, estimatedKu: number): Promise<void> {
    const usage = await this.billingClientService.getUsage(workspaceId)

    if (usage?.billingModel === 'overage' && usage.remaining !== null && estimatedKu > usage.remaining) {
      throw new HttpException(
        {
          message:
            'Creating this embedding index would push the workspace into overage billing. Reduce KU usage or wait for the next billing period before enabling backfill.',
          success: false,
          usage
        },
        HttpStatus.PAYMENT_REQUIRED
      )
    }

    const limitsCheck = await this.checkLimits(workspaceId, { estimatedKu })

    if (!limitsCheck.allowed || limitsCheck.usage.willIncurOverage) {
      throw new HttpException(
        {
          message:
            limitsCheck.reason ??
            'Creating this embedding index is blocked by the current KU billing policy.',
          success: false,
          usage: limitsCheck.usage
        },
        HttpStatus.PAYMENT_REQUIRED
      )
    }
  }

  async assertEmbeddingBatchAllowed(workspaceId: string, estimatedKu: number): Promise<void> {
    const limitsCheck = await this.checkLimits(workspaceId, { estimatedKu })

    if (!limitsCheck.allowed || limitsCheck.usage.willIncurOverage) {
      throw new HttpException(
        {
          message: limitsCheck.reason ?? 'would exceed allowed KU policy',
          success: false,
          usage: limitsCheck.usage
        },
        HttpStatus.PAYMENT_REQUIRED
      )
    }
  }
}
