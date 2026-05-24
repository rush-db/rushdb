import {
  Controller,
  Delete,
  Get,
  HttpCode,
  HttpStatus,
  Param,
  Post,
  Query,
  Request,
  UseInterceptors
} from '@nestjs/common'
import { ApiBearerAuth, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { toBoolean } from '@/common/utils/toBolean'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { PreferredTransactionDecorator } from '@/database/preferred-transaction.decorator'

import { RelationshipPatternsService } from './relationship-patterns.service'
import { RelationshipPatternDto, RelationshipPatternListResponse } from './relationship-patterns.types'

@Controller('relationships/patterns')
@ApiTags('Relationship Patterns')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, DataInterceptor)
export class RelationshipPatternsController {
  constructor(private readonly relationshipPatternsService: RelationshipPatternsService) {}

  @Get()
  @ApiBearerAuth()
  @AuthGuard('project')
  async list(
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<RelationshipPatternListResponse> {
    return this.relationshipPatternsService.list(request.projectId, transaction)
  }

  @Post('/analyze')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.ACCEPTED)
  async analyze(@Request() request: PlatformRequest): Promise<{ queued: true }> {
    await this.relationshipPatternsService.forceAnalysis(request.projectId, request.workspaceId)
    return { queued: true }
  }

  @Post('/:id/approve')
  @ApiBearerAuth()
  @AuthGuard('project')
  async approve(
    @Param('id') id: string,
    @Request() request: PlatformRequest
  ): Promise<RelationshipPatternDto | undefined> {
    return this.relationshipPatternsService.approve(request.projectId, id)
  }

  @Post('/:id/ignore')
  @ApiBearerAuth()
  @AuthGuard('project')
  async ignore(
    @Param('id') id: string,
    @Request() request: PlatformRequest
  ): Promise<RelationshipPatternDto | undefined> {
    return this.relationshipPatternsService.ignore(request.projectId, id)
  }

  @Delete('/:id')
  @ApiBearerAuth()
  @AuthGuard('project')
  async delete(
    @Param('id') id: string,
    @Request() request: PlatformRequest,
    @Query('deleteExisting') deleteExisting?: string
  ): Promise<{ deleted: true }> {
    await this.relationshipPatternsService.delete(request.projectId, id, toBoolean(deleteExisting))
    return { deleted: true }
  }
}
