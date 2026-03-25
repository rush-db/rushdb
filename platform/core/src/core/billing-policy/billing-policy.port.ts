import { CheckLimitsResponse } from '@/core/billing-client/billing-client.types'

export interface BillingLimitOptions {
  estimatedKu?: number
  projectCount?: number
  userCount?: number
}

export interface BillingPolicyPort {
  checkLimits(workspaceId: string, options?: BillingLimitOptions): Promise<CheckLimitsResponse>
  isPlanActive(workspaceId: string): Promise<boolean>
  assertProjectOperationAllowed(workspaceId: string, options: BillingLimitOptions): Promise<void>
  assertEmbeddingIndexCreationAllowed(workspaceId: string, estimatedKu: number): Promise<void>
  assertEmbeddingBatchAllowed(workspaceId: string, estimatedKu: number): Promise<void>
}

export const BILLING_POLICY_PORT = Symbol('BILLING_POLICY_PORT')
