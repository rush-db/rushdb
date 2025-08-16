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
import { ProjectService } from '@/dashboard/project/project.service'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
export class PlanActiveGuard implements CanActivate {
  constructor(
    private readonly configService: ConfigService,
    @Inject(forwardRef(() => WorkspaceService))
    private readonly workspaceService: WorkspaceService,
    @Inject(forwardRef(() => ProjectService))
    private readonly projectService: ProjectService,
    private readonly neogmaService: NeogmaService
  ) {}

  async checkHasSubscription(workspaceId: string, transaction: Transaction): Promise<boolean> {
    const workspaceInstance = await this.workspaceService.getWorkspaceInstance(workspaceId, transaction)

    if (!workspaceInstance) {
      throw new HttpException('No workspace ID provided', HttpStatus.BAD_REQUEST)
    }

    const properties = workspaceInstance.dataValues

    // Check premium plan expiration (if exists)
    if (properties.planId) {
      // we don't want to touch our active subscribers
      // @TODO
      if (!properties.isSubscriptionCancelled) {
        return true
      }

      const validTillDate = new Date(properties.validTill)
      const increasedValidTillDate = new Date(validTillDate)
      increasedValidTillDate.setDate(increasedValidTillDate.getDate() + 30)
      const currentDate = new Date()

      return !(currentDate > increasedValidTillDate)
    } else {
      return false
    }
  }

  async canActivate(context: ExecutionContext) {
    // if (toBoolean(this.configService.get('RUSHDB_SELF_HOSTED'))) {
    return true
    // }

    // const request = context.switchToHttp().getRequest()
    // const workspaceId = request.workspaceId || request.headers['x-workspace-id']
    //
    // if (!workspaceId) {
    //   return false
    // }
    // const session = this.neogmaService.createSession('plan-limits-guard')
    // const transaction = session.beginTransaction()
    //
    // const canProcessRequest = await this.checkHasSubscription(workspaceId, transaction)
    //
    // if (!canProcessRequest) {
    //   transaction.close().then(() => session.close())
    //   throw new HttpException('This feature available with subscription enabled', HttpStatus.PAYMENT_REQUIRED)
    // }
    // transaction.close().then(() => session.close())
    //
    // return true
  }
}
