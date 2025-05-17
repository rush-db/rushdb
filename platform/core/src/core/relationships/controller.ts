import {
  Body,
  Controller,
  DefaultValuePipe,
  HttpCode,
  HttpStatus,
  Param,
  ParseIntPipe,
  Post,
  Put,
  Query,
  Request,
  UseGuards,
  UseInterceptors,
  UsePipes
} from '@nestjs/common'
import { ApiBearerAuth, ApiParam, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { ValidationPipe } from '@/common/validation/validation.pipe'
import { RUSHDB_KEY_ID, RUSHDB_KEY_PROJECT_ID } from '@/core/common/constants'
import { EntityService } from '@/core/entity/entity.service'
import { TRecordRelationsResponse } from '@/core/entity/entity.types'
import { AttachDto } from '@/core/relationships/dto/attach.dto'
import { DetachDto } from '@/core/relationships/dto/detach.dto'
import {
  createRelationSchema,
  deleteRelationsSchema
} from '@/core/relationships/validation/schemas/relations.schema'
import { SearchDto } from '@/core/search/dto/search.dto'
import { pagination } from '@/core/search/parser/pagination'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { IsRelatedToProjectGuard } from '@/dashboard/auth/guards/is-related-to-project.guard'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaTransactionInterceptor } from '@/database/neogma/neogma-transaction.interceptor'
import { CustomTransactionInterceptor } from '@/database/neogma-dynamic/custom-transaction.interceptor'
import { PreferredTransactionDecorator } from '@/database/neogma-dynamic/preferred-transaction.decorator'

// @TODO: deprecate /:entityId based endpoints in prior of source / target SearchQuery-based approach
// for example: { source: { where: { $id: ... } }, target: { where: { $id: ... } }, type?: string }
// no direction would be needed with this approach as we explicitly defining source & target

@Controller('relationships')
@ApiTags('Relationships')
@UseInterceptors(
  TransformResponseInterceptor,
  NotFoundInterceptor,
  NeogmaDataInterceptor,
  NeogmaTransactionInterceptor,
  CustomTransactionInterceptor
)
export class RelationshipsController {
  constructor(private readonly entityService: EntityService) {}

  @Post(':entityId')
  @ApiParam({
    name: 'entityId',
    required: true,
    description: 'entity identifier (UUIDv7)',
    type: 'string'
  })
  @ApiBearerAuth()
  @UseGuards(
    IsRelatedToProjectGuard(['targetIds'], {
      nodeProperty: RUSHDB_KEY_ID,
      projectIdProperty: RUSHDB_KEY_PROJECT_ID
    })
  )
  @UsePipes(ValidationPipe(createRelationSchema, 'body'))
  @AuthGuard('project')
  @HttpCode(HttpStatus.CREATED)
  async attach(
    @Param('entityId') entityId: string,
    @Body() attachDto: AttachDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<{ message: string }> {
    const projectId = request.projectId
    return await this.entityService.attach(entityId, attachDto, projectId, transaction)
  }

  @Put(':entityId')
  @ApiParam({
    name: 'entityId',
    required: true,
    description: 'entity identifier (UUIDv7)',
    type: 'string'
  })
  @ApiBearerAuth()
  @UseGuards(
    IsRelatedToProjectGuard(['targetIds'], {
      nodeProperty: RUSHDB_KEY_ID,
      projectIdProperty: RUSHDB_KEY_PROJECT_ID
    })
  )
  @UsePipes(ValidationPipe(deleteRelationsSchema, 'body'))
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async detach(
    @Param('entityId') entityId: string,
    @Body() detachDto: DetachDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<{ message: string }> {
    const projectId = request.projectId
    return await this.entityService.detach(entityId, detachDto, projectId, transaction)
  }

  // @TODO: Remove it in prior of making separate  api
  @Post('/search')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async findRelations(
    @PreferredTransactionDecorator() transaction: Transaction,
    @Body() searchQuery: SearchDto = {},
    @Request() request: PlatformRequest,
    @Query('skip', new DefaultValuePipe(0), ParseIntPipe) skip?: number,
    @Query('limit', new DefaultValuePipe(1000), ParseIntPipe) limit?: number
  ): Promise<TRecordRelationsResponse> {
    const projectId = request.projectId

    const [data, total] = await Promise.all([
      this.entityService.findRelations({
        searchQuery,
        pagination: pagination(skip, limit),
        projectId,
        transaction
      }),
      this.entityService.findRelationsCount({
        projectId,
        searchQuery,
        transaction
      })
    ])

    return {
      data,
      total
    }
  }
}
