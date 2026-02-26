import {
  CanActivate,
  ExecutionContext,
  forwardRef,
  HttpException,
  HttpStatus,
  Inject,
  Injectable
} from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { Transaction } from 'neo4j-driver'

import { toBoolean } from '@/common/utils/toBolean'
import { BillingClientService } from '@/core/billing-client/billing-client.service'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'

@Injectable()
export class PlanActiveGuard implements CanActivate {
  constructor(
    private readonly configService: ConfigService,
    private readonly billingClientService: BillingClientService,
    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService
  ) {}

  async checkHasSubscription(workspaceId: string, transaction: Transaction): Promise<boolean> {
    // Check subscription status via billing service
    const customer = await this.billingClientService.getCustomer(workspaceId)

    if (!customer) {
      // No customer record = not subscribed
      return false
    }

    // Check if customer has an active paid subscription
    if (customer.subscriptionStatus === 'active' && customer.subscriptionId) {
      return true
    }

    // Free tier doesn't have active subscription
    return customer.plan !== 'free'
  }

  async canActivate(context: ExecutionContext) {
    if (toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
      return true
    }

    const request = context.switchToHttp().getRequest()
    const workspaceId = request.workspaceId

    if (!workspaceId) {
      return false
    }
    const transaction = request.transaction || request.raw?.transaction

    const canProcessRequest = await this.checkHasSubscription(workspaceId, transaction)

    if (!canProcessRequest) {
      throw new HttpException('This feature available with subscription enabled', HttpStatus.PAYMENT_REQUIRED)
    }

    return true
  }
}
