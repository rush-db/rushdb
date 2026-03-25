import { Injectable } from '@nestjs/common'

import { BillingAccountPort } from '@/core/billing-policy/billing-account.port'

@Injectable()
export class SelfHostedBillingAccountService implements BillingAccountPort {
  async createWorkspaceCustomer(_workspaceId: string, _ownerEmail?: string | null): Promise<null> {
    return null
  }

  async deleteWorkspaceCustomer(_workspaceId: string): Promise<boolean> {
    return true
  }

  async getWorkspaceCustomer(_workspaceId: string): Promise<null> {
    return null
  }

  async getWorkspaceUsage(_workspaceId: string): Promise<null> {
    return null
  }
}
