import {
  CanActivate,
  ExecutionContext,
  forwardRef,
  HttpException,
  HttpStatus,
  Inject,
  Injectable
} from '@nestjs/common'
import { FastifyRequest } from 'fastify'
import { Transaction } from 'neo4j-driver'

import { BILLING_POLICY_PORT, BillingPolicyPort } from '@/core/billing-policy/billing-policy.port'
import { CheckLimitsResponse } from '@/core/billing-client/billing-client.types'
import { ProjectService } from '@/dashboard/project/project.service'

/**
 * PlanLimitsGuard - enforces billing and operational limits.
 *
 * All limits (KU, projects, users) are now managed by the billing service.
 * The platform queries the billing service for all limit checks.
 *
 * Self-hosted mode bypasses all limits.
 */
@Injectable()
export class PlanLimitsGuard implements CanActivate {
  constructor(
    @Inject(BILLING_POLICY_PORT)
    private readonly billingPolicyService: BillingPolicyPort,
    @Inject(forwardRef(() => ProjectService))
    private readonly projectService: ProjectService
  ) {}

  async checkLimits(
    workspaceId: string,
    request: FastifyRequest,
    transaction: Transaction
  ): Promise<CheckLimitsResponse> {
    // Get current counts for operational limits
    const workspaceSummaryState = await this.projectService.getProjectsProperties(workspaceId, transaction)
    const projectCount = workspaceSummaryState.length

    // TODO: Implement user count when user management is ready
    // const userCount = await this.workspaceService.getUsersCount(workspaceId, transaction)

    // Check all limits (KU + operational) via billing service
    const limitsCheck = await this.billingPolicyService.checkLimits(workspaceId, {
      projectCount
      // userCount  // Uncomment when user management is implemented
    })

    return limitsCheck
  }

  async canActivate(context: ExecutionContext) {
    const request = context.switchToHttp().getRequest()
    const workspaceId = request.workspaceId

    if (!workspaceId) {
      return false
    }

    const transaction = request.transaction || request.raw?.transaction

    const limitsCheck = await this.checkLimits(workspaceId, request, transaction)

    if (!limitsCheck.allowed) {
      throw new HttpException(
        {
          message: limitsCheck.reason ?? 'Plan limits exceeded',
          success: false,
          usage: limitsCheck.usage
        },
        HttpStatus.PAYMENT_REQUIRED
      )
    }

    return true
  }
}
