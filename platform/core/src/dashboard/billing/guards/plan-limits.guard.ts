import {
  CanActivate,
  ExecutionContext,
  forwardRef,
  HttpException,
  HttpStatus,
  Inject,
  Injectable,
  PayloadTooLargeException
} from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { FastifyRequest } from 'fastify'
import { Transaction } from 'neo4j-driver'

import { toBoolean } from '@/common/utils/toBolean'
import { ProjectService } from '@/dashboard/project/project.service'
import { TWorkspaceLimits } from '@/dashboard/workspace/model/workspace.interface'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
export class PlanLimitsGuard implements CanActivate {
  constructor(
    private readonly configService: ConfigService,
    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService,
    @Inject(forwardRef(() => ProjectService))
    private readonly projectService: ProjectService,
    private readonly neogmaService: NeogmaService
  ) {}

  async checkLimits(
    workspaceId: string,
    request: FastifyRequest,
    transaction: Transaction
  ): Promise<boolean> {
    const workspaceInstance = await this.workspaceService.getWorkspaceInstance(workspaceId, transaction)
    const properties = workspaceInstance.dataValues
    const limits = JSON.parse(properties.limits) as TWorkspaceLimits

    const requestSize = Number(
      request?.raw?.headers?.['content-length'] ??
        request?.raw?.headers?.['Content-Length'] ??
        request?.headers?.['Content-Length'] ??
        request?.headers?.['content-length'] ??
        request?.socket?.bytesRead
    )

    // By default, we check import size limits
    // For binary data uploads we must check formdata and compare its size against limits.fileSize
    const targetLimit = limits.importSize

    // Check body size limits
    if (requestSize > targetLimit) {
      throw new PayloadTooLargeException(
        `Reduce size to ${targetLimit / 1024}KB. Got ${requestSize / 1024}KB`
      )
    }

    // Check premium plan expiration (if exists)
    if (properties.planId) {
      // we don't want to touch our active subscribers
      if (!properties.isSubscriptionCancelled) {
        return true
      }

      const validTillDate = new Date(properties.validTill)
      const increasedValidTillDate = new Date(validTillDate)
      increasedValidTillDate.setDate(increasedValidTillDate.getDate() + 30)
      const currentDate = new Date()

      return !(currentDate > increasedValidTillDate)
    }

    const workspaceSummaryState = await this.projectService.getProjectsProperties(workspaceId, transaction)
    const projectsCount = workspaceSummaryState.length

    const accumulatedWorkspaceStats = await this.workspaceService.getAccumulatedWorkspaceStats(
      workspaceInstance,
      transaction
    )

    // Check project count limits
    if (!accumulatedWorkspaceStats.records) {
      return !(limits.projects && projectsCount > limits.projects)
    }

    // Check Records limits
    return !(
      accumulatedWorkspaceStats.records >= limits.records ||
      (limits.projects && projectsCount > limits.projects)
    )
  }

  async canActivate(context: ExecutionContext) {
    if (toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
      return true
    }

    const request = context.switchToHttp().getRequest()
    const workspaceId = request.workspaceId || request.headers['x-workspace-id']

    if (!workspaceId) {
      return false
    }

    const session = this.neogmaService.createSession()
    const transaction = session.beginTransaction()

    const canProcessRequest = await this.checkLimits(workspaceId, request, transaction)

    if (!canProcessRequest) {
      transaction.close().then(() => session.close())
      throw new HttpException('Excess records or projects', HttpStatus.PAYMENT_REQUIRED)
    }
    transaction.close().then(() => session.close())

    return true
  }
}
