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

import { isNumeric } from '@/common/utils/isNumeric'
import { toBoolean } from '@/common/utils/toBolean'
import { CheckLimitsResponse } from '@/core/billing-client/billing-client.types'
import { BILLING_POLICY_PORT, BillingPolicyPort } from '@/core/billing-policy/billing-policy.port'
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

  private isProjectCreateRequest(request: FastifyRequest): boolean {
    const method = String((request as any)?.method ?? '').toUpperCase()
    const rawPath =
      ((request as any)?.routeOptions?.url as string | undefined) ??
      ((request as any)?.routerPath as string | undefined) ??
      ((request as any)?.raw?.url as string | undefined) ??
      ((request as any)?.url as string | undefined) ??
      ''
    const path = rawPath.split('?')[0]

    return method === 'POST' && /\/projects\/?$/.test(path)
  }

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

    if (!limitsCheck.allowed && this.isProjectCreateRequest(request)) {
      const projectLimit = limitsCheck?.limits?.projectLimit
      const projectCount = limitsCheck?.limits?.projectCount
      const projectLimitReached =
        toBoolean(projectLimit) && isNumeric(projectCount) && projectCount >= projectLimit

      // Project creation should be gated by operational limits, not KU exhaustion.
      if (!projectLimitReached) {
        return true
      }
    }

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
