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
import { Transaction } from 'neo4j-driver'

import { isDevMode } from '@/common/utils/isDevMode'
import { toBoolean } from '@/common/utils/toBolean'
import { ProjectService } from '@/dashboard/project/project.service'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import { dbContextStorage } from '@/database/db-context'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
export class CustomDbWriteRestrictionGuard implements CanActivate {
  constructor(
    private readonly configService: ConfigService,
    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService,
    @Inject(forwardRef(() => ProjectService))
    private readonly projectService: ProjectService,
    private readonly neogmaService: NeogmaService
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
    const workspaceId = request.workspaceId || request.headers['x-workspace-id']

    const dbContext = dbContextStorage.getStore()
    const externalDbConnection = dbContext.externalConnection

    if (!workspaceId) {
      return false
    }

    if (!externalDbConnection) {
      return true
    }

    isDevMode(() => Logger.log(`[CDWR GUARD]: Transaction created for CDWR guard`))

    const session = this.neogmaService.createSession('custom-db-write-restriction-guard')
    const transaction = session.beginTransaction()
    const canProcessRequest = true // await this.isCustomDbOptionEnabled(workspaceId, hasExternalDb, transaction)

    if (!canProcessRequest) {
      transaction.close().then(() => session.close())
      isDevMode(() => Logger.log(`[CDWR GUARD]: Close transaction and request due to billing settings`))
      throw new HttpException(
        'Cannot process data creation due to billing settings',
        HttpStatus.PAYMENT_REQUIRED
      )
    }

    isDevMode(() => Logger.log(`[CDWR GUARD]: Close transaction and process request`))
    transaction.close().then(() => session.close())

    return true
  }
}
