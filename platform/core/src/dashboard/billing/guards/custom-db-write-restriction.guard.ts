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
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import { dbContextStorage } from '@/database/db-context'

@Injectable()
export class CustomDbWriteRestrictionGuard implements CanActivate {
  constructor(
    private readonly configService: ConfigService,
    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService
  ) {}

  async isCustomDbOptionEnabled(
    workspaceId: string,
    hasExternalDb: boolean,
    transaction: Transaction
  ): Promise<boolean> {
    const workspaceInstance = await this.workspaceService.getWorkspaceInstance(workspaceId, transaction)
    const properties = workspaceInstance.dataValues

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

    return !hasExternalDb
  }

  async canActivate(context: ExecutionContext) {
    if (toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
      return true
    }

    const request = context.switchToHttp().getRequest()
    const workspaceId = request.workspaceId

    const dbContext = dbContextStorage.getStore()
    const externalDbConnection = dbContext.externalConnection

    if (!workspaceId) {
      return false
    }

    if (!externalDbConnection) {
      return true
    }

    const transaction = request.transaction || request.raw?.transaction
    const canProcessRequest = await this.isCustomDbOptionEnabled(
      workspaceId,
      toBoolean(externalDbConnection),
      transaction
    )

    if (!canProcessRequest) {
      throw new HttpException(
        'Cannot process data creation due to billing settings',
        HttpStatus.PAYMENT_REQUIRED
      )
    }

    return true
  }
}
