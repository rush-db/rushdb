import { Injectable } from '@nestjs/common'

import { BillingClientService } from '@/core/billing-client/billing-client.service'
import { BillingAccountPort } from '@/core/billing-policy/billing-account.port'

@Injectable()
export class CloudBillingAccountService implements BillingAccountPort {
  constructor(private readonly billingClientService: BillingClientService) {}

  async createWorkspaceCustomer(workspaceId: string, ownerEmail?: string | null, plan: string = 'free') {
    return this.billingClientService.createCustomer(workspaceId, plan, ownerEmail)
  }

  async deleteWorkspaceCustomer(workspaceId: string): Promise<boolean> {
    return this.billingClientService.deleteCustomer(workspaceId)
  }

  async getWorkspaceCustomer(workspaceId: string) {
    return this.billingClientService.getCustomer(workspaceId)
  }

  async getWorkspaceUsage(workspaceId: string) {
    return this.billingClientService.getUsage(workspaceId)
  }
}
