import { Injectable } from '@nestjs/common'

import { CheckLimitsResponse } from '@/core/billing-client/billing-client.types'
import { BillingLimitOptions, BillingPolicyPort } from '@/core/billing-policy/billing-policy.port'

@Injectable()
export class SelfHostedBillingPolicyService implements BillingPolicyPort {
  async checkLimits(_workspaceId: string, _options?: BillingLimitOptions): Promise<CheckLimitsResponse> {
    return {
      allowed: true,
      usage: { kuConsumed: 0, kuLimit: null, plan: 'self-hosted', remaining: null },
      limits: { projectLimit: null, userLimit: null }
    }
  }

  async isPlanActive(_workspaceId: string): Promise<boolean> {
    return true
  }

  async assertProjectOperationAllowed(_workspaceId: string, _options: BillingLimitOptions): Promise<void> {
    return
  }

  async assertEmbeddingIndexCreationAllowed(_workspaceId: string, _estimatedKu: number): Promise<void> {
    return
  }

  async assertEmbeddingBatchAllowed(_workspaceId: string, _estimatedKu: number): Promise<void> {
    return
  }

  async isSsoAllowed(_workspaceId: string): Promise<boolean> {
    return true
  }

  async assertSsoAllowed(_workspaceId: string): Promise<void> {
    return
  }
}
