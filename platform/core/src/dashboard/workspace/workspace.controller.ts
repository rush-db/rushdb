import {
  Body,
  Controller,
  Delete,
  Get,
  HttpCode,
  HttpStatus,
  Param,
  Patch,
  Post,
  UseInterceptors
} from '@nestjs/common'
import { ApiBearerAuth, ApiExcludeController, ApiParam, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { AuthUser } from '@/dashboard/user/decorators/user.decorator'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'
import { EditWorkspaceDto } from '@/dashboard/workspace/dto/edit-workspace.dto'
import { IWorkspaceProperties } from '@/dashboard/workspace/model/workspace.interface'
import { WorkspaceService } from '@/dashboard/workspace/workspace.service'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaTransactionInterceptor } from '@/database/neogma/neogma-transaction.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'
import { InviteToWorkspaceDto } from '@/dashboard/workspace/dto/invite-to-workspace.dto'
import { RecomputeAccessListDto } from '@/dashboard/workspace/dto/recompute-access-list.dto'
import { RevokeAccessDto } from '@/dashboard/workspace/dto/revoke-access.dto'

@Controller('workspaces')
@ApiExcludeController()
@UseInterceptors(
  TransformResponseInterceptor,
  NotFoundInterceptor,
  NeogmaDataInterceptor,
  NeogmaTransactionInterceptor,
  ChangeCorsInterceptor
)
export class WorkspaceController {
  constructor(private readonly workspaceService: WorkspaceService) {}

  // Don't allow to create more than 1 org. Temporary disabled (!)
  // @Post()
  // @ApiTags('Workspaces')
  // @ApiBearerAuth()
  // @HttpCode(HttpStatus.CREATED)
  // @UseGuards(JwtAuthGuard, WorkspacesCountGuard)
  // async create(
  //     @Body() workspaceProperties: CreateWorkspaceDto,
  //     @AuthUser() { id: userId }: IUserClaims,
  //     @TransactionDecorator() transaction: Transaction
  // ): Promise<IWorkspaceProperties> {
  //     const workspace = await this.workspaceService.createWorkspace(
  //         workspaceProperties,
  //         userId,
  //         transaction
  //     );
  //
  //     return workspace.toJson();
  // }

  @Get(':id')
  @ApiParam({
    name: 'id',
    required: true,
    description: 'workspace identifier (UUIDv7)',
    type: 'string'
  })
  @ApiTags('Workspaces')
  @ApiBearerAuth()
  @AuthGuard()
  async getWorkspace(
    @Param('id') id: string,
    @TransactionDecorator() transaction: Transaction
  ): Promise<IWorkspaceProperties | undefined> {
    const workspace = await this.workspaceService.getWorkspace(id, transaction)
    return workspace?.toJson()
  }

  @Get()
  @ApiTags('Workspaces')
  @ApiBearerAuth()
  @AuthGuard(null)
  async getWorkspacesList(
    @AuthUser() { id }: IUserClaims,
    @TransactionDecorator() transaction: Transaction
  ): Promise<IWorkspaceProperties[]> {
    return await this.workspaceService.getWorkspacesList(id, transaction)
  }

  @Patch(':id')
  @ApiParam({
    name: 'id',
    required: true,
    description: 'workspace identifier (UUIDv7)',
    type: 'string'
  })
  @ApiTags('Workspaces')
  @ApiBearerAuth()
  @AuthGuard()
  @HttpCode(HttpStatus.CREATED)
  async editWorkspace(
    @Param('id') id: string,
    @Body() workspacePayload: EditWorkspaceDto,
    @TransactionDecorator() transaction: Transaction
  ): Promise<IWorkspaceProperties | undefined> {
    const workspace = await this.workspaceService.patchWorkspace(id, workspacePayload, transaction)
    return workspace?.toJson()
  }

  @Delete(':id')
  @ApiParam({
    name: 'id',
    required: true,
    description: 'workspace identifier (UUIDv7)',
    type: 'string'
  })
  @ApiTags('Workspaces')
  @ApiBearerAuth()
  @AuthGuard()
  @HttpCode(HttpStatus.OK)
  async deleteWorkspace(
    @Param('id') id: string,
    @TransactionDecorator() transaction: Transaction
  ): Promise<{ message: string }> {
    return await this.workspaceService.deleteWorkspace(id, transaction)
  }

  @Post(':id/invite')
  @ApiParam({
    name: 'id',
    required: true,
    description: 'workspace identifier (UUIDv7)',
    type: 'string'
  })
  @ApiTags('Workspaces')
  @ApiBearerAuth()
  @AuthGuard()
  @HttpCode(HttpStatus.OK)
  async inviteToWorkspace(
    @AuthUser() user: IUserClaims,
    @Param('id') id: string,
    @Body() workspacePayload: InviteToWorkspaceDto,
    @TransactionDecorator() transaction: Transaction
  ): Promise<{ message: string }> {
    const { name } = await this.workspaceService.getWorkspaceInstance(id, transaction)

    const payload = {
      workspaceId: id,
      workspaceName: name,
      senderEmail: user.login,
      ...workspacePayload
    }
    return await this.workspaceService.inviteMember(payload, transaction)
  }

  @Patch(':id/access-list')
  @ApiParam({
    name: 'id',
    required: true,
    description: 'workspace identifier (UUIDv7)',
    type: 'string'
  })
  @ApiTags('Workspaces')
  @ApiBearerAuth()
  @AuthGuard()
  @HttpCode(HttpStatus.OK)
  async recomputeAccessList(
    @Param('id') id: string,
    @Body() accessMap: RecomputeAccessListDto,
    @TransactionDecorator() transaction: Transaction
  ): Promise<{ message: string }> {
    return await this.workspaceService.recomputeProjectsAccessList(id, accessMap, transaction)
  }

  @Get(':id/access-list')
  @ApiParam({
    name: 'id',
    required: true,
    description: 'workspace identifier (UUIDv7)',
    type: 'string'
  })
  @ApiTags('Workspaces')
  @ApiBearerAuth()
  @AuthGuard()
  @HttpCode(HttpStatus.OK)
  async getAccessList(@Param('id') id: string, @TransactionDecorator() transaction: Transaction) {
    return this.workspaceService.getAccessListByProjects(id, transaction)
  }

  @Get(':id/user-list')
  @ApiParam({
    name: 'id',
    required: true,
    description: 'workspace identifier (UUIDv7)',
    type: 'string'
  })
  @ApiTags('Workspaces')
  @ApiBearerAuth()
  @AuthGuard()
  @HttpCode(HttpStatus.OK)
  async getDeveloperUserList(@Param('id') id: string, @TransactionDecorator() transaction: Transaction) {
    return this.workspaceService.getInvitedUserList(id, transaction)
  }

  @Post(':id/revoke-access')
  @ApiParam({
    name: 'id',
    required: true,
    description: 'workspace identifier (UUIDv7)',
    type: 'string'
  })
  @ApiTags('Workspaces')
  @ApiBearerAuth()
  @AuthGuard()
  @HttpCode(HttpStatus.OK)
  async revokeAccess(
    @Param('id') id: string,
    @Body() { userIds }: RevokeAccessDto,
    @TransactionDecorator() transaction: Transaction
  ): Promise<{ message: string }> {
    return await this.workspaceService.revokeAccessList(id, userIds, transaction)
  }
}
