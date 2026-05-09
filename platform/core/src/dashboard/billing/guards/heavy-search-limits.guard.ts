import { CanActivate, ExecutionContext, HttpException, HttpStatus, Injectable } from '@nestjs/common'
import { Inject } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'

import { BILLING_POLICY_PORT, BillingPolicyPort } from '@/core/billing-policy/billing-policy.port'
import { isHeavySearch } from '@/core/ku-events/search-complexity'
import { ProjectService } from '@/dashboard/project/project.service'

/**
 * Enforces billing limits only for compute-heavy search payloads.
 * Non-heavy searches are always allowed.
 */
@Injectable()
export class HeavySearchLimitsGuard implements CanActivate {
  constructor(
    @Inject(BILLING_POLICY_PORT)
    private readonly billingPolicyService: BillingPolicyPort,
    private readonly projectService: ProjectService
  ) {}

  async canActivate(context: ExecutionContext): Promise<boolean> {
    const request = context.switchToHttp().getRequest()
    const raw: any = (request as any).raw ?? request
    const workspaceId: string | undefined = request.workspaceId

    if (!workspaceId) {
      return false
    }

    // External DB projects manage their own compute infra; skip KU limits.
    if (raw.project?.customDb) {
      return true
    }

    const body = (request.body ?? {}) as Record<string, unknown>
    const heavy = isHeavySearch(body as any)
    if (!heavy) {
      return true
    }

    const transaction: Transaction = request.transaction || request.raw?.transaction
    const workspaceSummaryState = await this.projectService.getProjectsProperties(workspaceId, transaction)
    const projectCount = workspaceSummaryState.length

    const limitsCheck = await this.billingPolicyService.checkLimits(workspaceId, {
      projectCount
    })

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
