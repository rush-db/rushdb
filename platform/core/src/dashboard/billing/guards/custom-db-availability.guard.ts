import {
  CanActivate,
  ExecutionContext,
  forwardRef,
  HttpException,
  HttpStatus,
  Inject,
  Injectable,
  Logger
} from '@nestjs/common'
import { ConfigService } from '@nestjs/config'
import { FastifyRequest } from 'fastify'
import { Transaction } from 'neo4j-driver'

import { isDevMode } from '@/common/utils/isDevMode'
import { toBoolean } from '@/common/utils/toBolean'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
export class CustomDbAvailabilityGuard implements CanActivate {
  constructor(
    private readonly configService: ConfigService,
    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService,
    private readonly neogmaService: NeogmaService
  ) {}

  async isCustomDbOptionEnabled(
    workspaceId: string,
    request: FastifyRequest,
    transaction: Transaction
  ): Promise<boolean> {
    const workspaceInstance = await this.workspaceService.getWorkspaceInstance(workspaceId, transaction)
    const properties = workspaceInstance.dataValues
    const hasCustomDbProperty = Boolean(Object.keys(request.body?.['customDb'] ?? {}).length)

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

    return !hasCustomDbProperty
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

    isDevMode(() => Logger.log(`[CDA GUARD]: Transaction created for CDA guard`))

    const session = this.neogmaService.createSession('custom-db-write-availability-guard')
    const transaction = session.beginTransaction()

    const canProcessRequest = true //await this.isCustomDbOptionEnabled(workspaceId, request, transaction)

    if (!canProcessRequest) {
      transaction.close().then(() => session.close())
      isDevMode(() => Logger.log(`[CDA GUARD]: Close transaction and request due to billing settings`))
      throw new HttpException('Cannot attach custom db to the project', HttpStatus.PAYMENT_REQUIRED)
    }

    isDevMode(() => Logger.log(`[CDA GUARD]: Close transaction and process request`))
    transaction.close().then(() => session.close())

    return true
  }
}
