import {
  Body,
  Controller,
  Delete,
  Get,
  Headers,
  HttpCode,
  HttpStatus,
  Param,
  Patch,
  Post,
  Request,
  UseInterceptors
} from '@nestjs/common'
import { ApiBearerAuth, ApiExcludeController, ApiTags } from '@nestjs/swagger'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { ConnectorService } from '@/dashboard/connector/connector.service'
import { CreateConnectorDto, UpdateConnectorDto } from '@/dashboard/connector/dto/create-connector.dto'
import {
  ConnectorHeartbeatDto,
  ConnectorOffsetDto,
  ConnectorStatusDto
} from '@/dashboard/connector/dto/worker-connector.dto'
import { AuthUser } from '@/dashboard/user/decorators/user.decorator'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'

@Controller()
@ApiExcludeController()
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, DataInterceptor, ChangeCorsInterceptor)
export class ConnectorController {
  constructor(private readonly connectorService: ConnectorService) {}

  @Post('connectors/_internal/claim')
  claim(
    @Headers('x-synx-worker-id') workerId = 'synx-worker',
    @Headers('x-synx-lease-ttl-ms') leaseTtlMs?: string,
    @Headers('x-synx-control-token') token?: string
  ) {
    return this.connectorService.claim(workerId, token, Number(leaseTtlMs))
  }

  @Post('connectors/_internal/:connectorId/heartbeat')
  heartbeat(
    @Param('connectorId') connectorId: string,
    @Body() dto: ConnectorHeartbeatDto,
    @Headers('x-synx-worker-id') workerId = 'synx-worker',
    @Headers('x-synx-control-token') token?: string
  ) {
    return this.connectorService.heartbeat(connectorId, workerId, dto, token)
  }

  @Post('connectors/_internal/:connectorId/release')
  release(
    @Param('connectorId') connectorId: string,
    @Headers('x-synx-worker-id') workerId = 'synx-worker',
    @Headers('x-synx-control-token') token?: string
  ) {
    return this.connectorService.release(connectorId, workerId, token)
  }

  @Post('connectors/_internal/:connectorId/status')
  updateStatus(
    @Param('connectorId') connectorId: string,
    @Body() dto: ConnectorStatusDto,
    @Headers('x-synx-worker-id') workerId = 'synx-worker',
    @Headers('x-synx-control-token') token?: string
  ) {
    return this.connectorService.updateStatus(connectorId, dto, token, workerId)
  }

  @Post('connectors/_internal/:connectorId/offsets')
  commitOffset(
    @Param('connectorId') connectorId: string,
    @Body() dto: ConnectorOffsetDto,
    @Headers('x-synx-worker-id') workerId = 'synx-worker',
    @Headers('x-synx-control-token') token?: string
  ) {
    return this.connectorService.commitOffset(connectorId, dto, token, workerId)
  }

  @Post('connectors')
  @ApiTags('Connectors')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.CREATED)
  create(
    @Body() dto: CreateConnectorDto,
    @Request() request: PlatformRequest,
    @AuthUser() user: IUserClaims
  ) {
    return this.connectorService.create(dto, request.projectId, user?.id)
  }

  @Get('connectors')
  @ApiTags('Connectors')
  @ApiBearerAuth()
  @AuthGuard('project')
  list(@Request() request: PlatformRequest) {
    return this.connectorService.list(request.projectId)
  }

  @Get('connectors/:connectorId')
  @ApiTags('Connectors')
  @ApiBearerAuth()
  @AuthGuard('project')
  get(@Param('connectorId') connectorId: string, @Request() request: PlatformRequest) {
    return this.connectorService.get(connectorId, request.projectId)
  }

  @Patch('connectors/:connectorId')
  @ApiTags('Connectors')
  @ApiBearerAuth()
  @AuthGuard('project')
  update(
    @Param('connectorId') connectorId: string,
    @Body() dto: UpdateConnectorDto,
    @Request() request: PlatformRequest
  ) {
    return this.connectorService.update(connectorId, request.projectId, dto)
  }

  @Delete('connectors/:connectorId')
  @ApiTags('Connectors')
  @ApiBearerAuth()
  @AuthGuard('project')
  delete(@Param('connectorId') connectorId: string, @Request() request: PlatformRequest) {
    return this.connectorService.delete(connectorId, request.projectId)
  }

  @Post('connectors/:connectorId/test')
  @ApiTags('Connectors')
  @ApiBearerAuth()
  @AuthGuard('project')
  test(@Param('connectorId') connectorId: string, @Request() request: PlatformRequest) {
    return this.connectorService.test(connectorId, request.projectId)
  }

  @Post('connectors/:connectorId/pause')
  @ApiTags('Connectors')
  @ApiBearerAuth()
  @AuthGuard('project')
  pause(@Param('connectorId') connectorId: string, @Request() request: PlatformRequest) {
    return this.connectorService.setLifecycle(connectorId, request.projectId, 'paused')
  }

  @Post('connectors/:connectorId/resume')
  @ApiTags('Connectors')
  @ApiBearerAuth()
  @AuthGuard('project')
  resume(@Param('connectorId') connectorId: string, @Request() request: PlatformRequest) {
    return this.connectorService.setLifecycle(connectorId, request.projectId, 'running')
  }

  @Post('connectors/:connectorId/resnapshot')
  @ApiTags('Connectors')
  @ApiBearerAuth()
  @AuthGuard('project')
  resnapshot(@Param('connectorId') connectorId: string, @Request() request: PlatformRequest) {
    return this.connectorService.resnapshot(connectorId, request.projectId)
  }

  @Get('connectors/:connectorId/events')
  @ApiTags('Connectors')
  @ApiBearerAuth()
  @AuthGuard('project')
  events(@Param('connectorId') connectorId: string, @Request() request: PlatformRequest) {
    return this.connectorService.events(connectorId, request.projectId)
  }
}
