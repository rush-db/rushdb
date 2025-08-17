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
  UseGuards,
  UseInterceptors,
  Headers,
  Query
} from '@nestjs/common'
import { ApiBearerAuth, ApiExcludeController, ApiParam, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { CustomDbAvailabilityGuard } from '@/dashboard/billing/guards/custom-db-availability.guard'
import { PlanLimitsGuard } from '@/dashboard/billing/guards/plan-limits.guard'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { CreateProjectDto } from '@/dashboard/project/dto/create-project.dto'
import { UpdateProjectDto } from '@/dashboard/project/dto/update-project.dto'
import { ProjectEntity } from '@/dashboard/project/entity/project.entity'
import { IProjectProperties, TProjectProperties } from '@/dashboard/project/model/project.interface'
import { ProjectService } from '@/dashboard/project/project.service'
import { AuthUser } from '@/dashboard/user/decorators/user.decorator'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'

@Controller('projects')
@ApiExcludeController()
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, DataInterceptor, ChangeCorsInterceptor)
export class ProjectController {
  constructor(private readonly projectService: ProjectService) {}

  @Post()
  @ApiTags('Projects')
  @ApiBearerAuth()
  @HttpCode(HttpStatus.CREATED)
  @UseGuards(PlanLimitsGuard, CustomDbAvailabilityGuard)
  @AuthGuard('workspace', 'owner')
  async createProject(
    @Body() project: CreateProjectDto,
    @AuthUser() { id: userId }: IUserClaims,
    @Headers() headers,
    @TransactionDecorator() transaction: Transaction
  ): Promise<TProjectProperties> {
    const workspaceId = headers['x-workspace-id']

    if (!workspaceId) {
      return
    }

    const projectEntity = await this.projectService.createProject(project, workspaceId, userId, transaction)

    return projectEntity?.toJson()
  }

  @Get()
  @ApiTags('Projects')
  @ApiBearerAuth()
  @AuthGuard()
  @HttpCode(HttpStatus.OK)
  async getProjectsList(
    @AuthUser() { id: userId }: IUserClaims,
    @Headers() headers,
    @TransactionDecorator() transaction: Transaction
  ): Promise<ProjectEntity[]> {
    const workspaceId = headers['x-workspace-id']
    return await this.projectService.getProjectsByWorkspaceId(workspaceId, userId, transaction)
  }

  @Delete(':projectId')
  @ApiParam({
    name: 'projectId',
    required: true,
    description: 'project identifier (UUIDv7)',
    type: 'string'
  })
  @ApiTags('Projects')
  @ApiBearerAuth()
  // @UseInterceptors(RunSideEffectMixin([ESideEffectType.DELETE_PROJECT]))
  @AuthGuard('project', 'owner')
  async deleteProject(
    @Param('projectId') id: string,
    @Query('shouldStoreCustomDbData') shouldStoreCustomDbData: boolean,
    @TransactionDecorator() transaction: Transaction
  ): Promise<boolean> {
    return await this.projectService.deleteProject(id, transaction, shouldStoreCustomDbData)
  }

  @Patch(':projectId')
  @ApiParam({
    name: 'projectId',
    required: true,
    description: 'project identifier (UUIDv7)',
    type: 'string'
  })
  @ApiTags('Projects')
  @ApiBearerAuth()
  @HttpCode(HttpStatus.CREATED)
  @UseGuards(CustomDbAvailabilityGuard)
  @AuthGuard('project', 'owner')
  async updateProject(
    @Param('projectId') id: string,
    @Body() editProjectProperties: UpdateProjectDto,
    @TransactionDecorator() transaction: Transaction
  ): Promise<IProjectProperties> {
    const project = await this.projectService.updateProject(id, editProjectProperties, transaction)
    return project.toJson()
  }

  @Get(':projectId')
  @ApiParam({
    name: 'projectId',
    required: true,
    description: 'project identifier (UUIDv7)',
    type: 'string'
  })
  @ApiTags('Projects')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async getProject(
    @Param('projectId') id: string,
    @TransactionDecorator() transaction: Transaction
  ): Promise<IProjectProperties> {
    const project = await this.projectService.getProject(id, transaction)
    return project.toJson()
  }
}
