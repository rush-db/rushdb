import { CanActivate, ExecutionContext, HttpException, HttpStatus, Inject, Injectable } from '@nestjs/common'

import { BILLING_POLICY_PORT, BillingPolicyPort } from '@/core/billing-policy/billing-policy.port'

@Injectable()
export class PlanActiveGuard implements CanActivate {
  constructor(
    @Inject(BILLING_POLICY_PORT)
    private readonly billingPolicyService: BillingPolicyPort
  ) {}

  async checkHasSubscription(workspaceId: string): Promise<boolean> {
    return this.billingPolicyService.isPlanActive(workspaceId)
  }

  async canActivate(context: ExecutionContext) {
    const request = context.switchToHttp().getRequest()
    const workspaceId = request.workspaceId

    if (!workspaceId) {
      return false
    }

    const canProcessRequest = await this.checkHasSubscription(workspaceId)

    if (!canProcessRequest) {
      throw new HttpException('This feature available with subscription enabled', HttpStatus.PAYMENT_REQUIRED)
    }

    return true
  }
}
