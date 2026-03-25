import { Customer, UsageResponse } from '@/core/billing-client/billing-client.types'

export interface BillingAccountPort {
  createWorkspaceCustomer(
    workspaceId: string,
    ownerEmail?: string | null,
    plan?: string
  ): Promise<Customer | null>
  deleteWorkspaceCustomer(workspaceId: string): Promise<boolean>
  getWorkspaceCustomer(workspaceId: string): Promise<Customer | null>
  getWorkspaceUsage(workspaceId: string): Promise<UsageResponse | null>
}

export const BILLING_ACCOUNT_PORT = Symbol('BILLING_ACCOUNT_PORT')
