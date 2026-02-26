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
import { FastifyRequest } from 'fastify'
import { Transaction } from 'neo4j-driver'

import { toBoolean } from '@/common/utils/toBolean'
import { BillingClientService } from '@/core/billing-client/billing-client.service'
import { ProjectService } from '@/dashboard/project/project.service'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'

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
    private readonly configService: ConfigService,
    private readonly billingClientService: BillingClientService,
    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService,
    @Inject(forwardRef(() => ProjectService))
    private readonly projectService: ProjectService
  ) {}

  async checkLimits(
    workspaceId: string,
    request: FastifyRequest,
    transaction: Transaction
  ): Promise<boolean> {
    // Get current counts for operational limits
    const workspaceSummaryState = await this.projectService.getProjectsProperties(workspaceId, transaction)
    const projectCount = workspaceSummaryState.length

    // TODO: Implement user count when user management is ready
    // const userCount = await this.workspaceService.getUsersCount(workspaceId, transaction)

    // Check all limits (KU + operational) via billing service
    const limitsCheck = await this.billingClientService.checkLimits(workspaceId, {
      projectCount
      // userCount  // Uncomment when user management is implemented
    })

    return limitsCheck.allowed
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

    const canProcessRequest = await this.checkLimits(workspaceId, request, transaction)

    if (!canProcessRequest) {
      throw new HttpException('Plan limits exceeded', HttpStatus.PAYMENT_REQUIRED)
    }

    return true
  }
}
