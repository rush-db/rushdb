import { CanActivate, ExecutionContext, HttpException, HttpStatus, Injectable } from '@nestjs/common'

import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
export class WorkspacesCountGuard implements CanActivate {
  constructor(
    private readonly workspaceService: WorkspaceService,
    private readonly neogmaService: NeogmaService
  ) {}

  private async validateRequest(request: any): Promise<boolean> {
    const userId: string = request?.user?.id
    const session = this.neogmaService.createSession('workspaces-count-guard')
    const transaction = session.beginTransaction({ timeout: 30_000 })

    const workspaces = await this.workspaceService.getWorkspacesList(userId, transaction)

    const canActivate = workspaces.length <= 2

    if (canActivate) {
      await transaction.close()
      await this.neogmaService.closeSession(session, 'workspaces-count-guard (1)')
      return canActivate
    } else {
      await transaction.close()
      await this.neogmaService.closeSession(session, 'workspaces-count-guard (2)')
      throw new HttpException('You exceeded workspaces limit (3)', HttpStatus.BAD_REQUEST)
    }
  }
  async canActivate(context: ExecutionContext): Promise<boolean> {
    const request = context.switchToHttp().getRequest()
    return await this.validateRequest(request)
  }
}
