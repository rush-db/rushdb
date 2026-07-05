import {
  Body,
  Controller,
  HttpCode,
  HttpStatus,
  Param,
  Post,
  Put,
  Request,
  UseGuards,
  UseInterceptors,
  UsePipes
} from '@nestjs/common'
import { ApiBearerAuth, ApiParam, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { ESideEffectType, RunSideEffectMixin } from '@/common/interceptors/run-side-effect.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { ValidationPipe } from '@/common/validation/validation.pipe'
import { RUSHDB_KEY_ID, RUSHDB_KEY_PROJECT_ID } from '@/core/common/constants'
import { Where } from '@/core/common/types'
import { EntityService } from '@/core/entity/entity.service'
import { TRecordRelationsResponse, TRelationDirection } from '@/core/entity/entity.types'
import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { TrackHeavySearchKu } from '@/core/ku-events/track-heavy-search-ku.interceptor'
import { AttachDto } from '@/core/relationships/dto/attach.dto'
import { DetachDto } from '@/core/relationships/dto/detach.dto'
import { RelationshipSearchDto } from '@/core/relationships/dto/relationship-search.dto'
import { RelationshipProperties } from '@/core/relationships/relationship-properties'
import {
  createRelationSchema,
  deleteRelationsSchema,
  createRelationsByKeysSchema
} from '@/core/relationships/validation/schemas/relations.schema'
import { pagination } from '@/core/search/parser/pagination'
import { TokenReadAccess } from '@/dashboard/auth/decorators/token-read-access.decorator'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { IsRelatedToProjectGuard } from '@/dashboard/auth/guards/is-related-to-project.guard'
import { HeavySearchLimitsGuard } from '@/dashboard/billing/guards/heavy-search-limits.guard'
import { PlanLimitsGuard } from '@/dashboard/billing/guards/plan-limits.guard'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { PreferredTransactionDecorator } from '@/database/preferred-transaction.decorator'

@Controller('relationships')
@ApiTags('Relationships')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, DataInterceptor)
export class RelationshipsController {
  constructor(
    private readonly entityService: EntityService,
    private readonly kuEventsService: KuEventsService
  ) {}

  /**
   * Meters relationship graph writes for RushDB-managed (shared) instances,
   * mirroring how the import path and pattern-apply meter the same writes.
   * External-DB projects (customDb) manage their own infra and are not charged;
   * KuEventsService itself no-ops when self-hosted or billing is unconfigured.
   */
  private trackRelationshipKu(
    request: PlatformRequest,
    operation: KuOperation,
    count: number,
    metadata: Record<string, unknown>
  ) {
    if (count <= 0) {
      return
    }
    const raw: any = (request as any).raw ?? request
    const projectId: string = request.projectId ?? raw.projectId
    const workspaceId: string = (request as any).workspaceId ?? raw.workspaceId
    if (!raw.project?.customDb && workspaceId) {
      this.kuEventsService.emitBulk(workspaceId, projectId, operation, count, metadata)
    }
  }

  @Post(':entityId')
  @ApiParam({
    name: 'entityId',
    required: true,
    description: 'entity identifier (UUIDv7)',
    type: 'string'
  })
  @ApiBearerAuth()
  @UseGuards(
    PlanLimitsGuard,
    IsRelatedToProjectGuard(['targetIds'], {
      nodeProperty: RUSHDB_KEY_ID,
      projectIdProperty: RUSHDB_KEY_PROJECT_ID
    })
  )
  @UsePipes(ValidationPipe(createRelationSchema, 'body'))
  @AuthGuard('project')
  @UseInterceptors(RunSideEffectMixin([ESideEffectType.RECALCULATE_SCHEMA_CACHE]))
  @HttpCode(HttpStatus.CREATED)
  async attach(
    @Param('entityId') entityId: string,
    @Body() attachDto: AttachDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<{ message: string }> {
    const projectId = request.projectId
    const normalizedDto: AttachDto = {
      ...attachDto,
      direction: attachDto.direction?.toLowerCase() as TRelationDirection | undefined
    }
    const result = await this.entityService.attach(entityId, normalizedDto, projectId, transaction)
    this.trackRelationshipKu(
      request,
      KuOperation.RELATIONSHIP_CREATED,
      Array.isArray(attachDto.targetIds) ? attachDto.targetIds.length : 1,
      { trigger: 'relationships_attach', type: attachDto.type }
    )
    return result
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
  @UseInterceptors(RunSideEffectMixin([ESideEffectType.RECALCULATE_SCHEMA_CACHE]))
  @HttpCode(HttpStatus.OK)
  async detach(
    @Param('entityId') entityId: string,
    @Body() detachDto: DetachDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<{ message: string }> {
    const projectId = request.projectId
    const normalizedDto: DetachDto = {
      ...detachDto,
      direction: detachDto.direction?.toLowerCase() as TRelationDirection | undefined
    }
    const result = await this.entityService.detach(entityId, normalizedDto, projectId, transaction)
    this.trackRelationshipKu(
      request,
      KuOperation.KNOWLEDGE_DELETED,
      Array.isArray(detachDto.targetIds) ? detachDto.targetIds.length : 1,
      { kind: 'relationship', trigger: 'relationships_detach' }
    )
    return result
  }

  // @TODO: deprecate /:entityId based endpoints in prior of source / target SearchQuery-based approach
  // for example: { source: { where: { $id: ... } }, target: { where: { $id: ... } }, type?: string }
  @Post('/create-many')
  @ApiBearerAuth()
  @UseGuards(PlanLimitsGuard, IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(createRelationsByKeysSchema, 'body'))
  @UseInterceptors(RunSideEffectMixin([ESideEffectType.RECALCULATE_SCHEMA_CACHE]))
  @HttpCode(HttpStatus.CREATED)
  async createMany(
    @Body()
    body: {
      source: { label: string; key?: string; where?: Where }
      target: { label: string; key?: string; where?: Where }
      type?: string
      direction?: TRelationDirection
      properties?: RelationshipProperties
      manyToMany?: boolean
    },
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<{ message: string }> {
    const projectId = request.projectId
    const normalizedDirection = body.direction?.toLowerCase() as TRelationDirection | undefined
    const createdCount = await this.entityService.createRelationsByKeys({
      ...body,
      direction: normalizedDirection,
      projectId,
      transaction,
      manyToMany: body.manyToMany
    })
    this.trackRelationshipKu(request, KuOperation.RELATIONSHIP_CREATED, createdCount, {
      trigger: 'relationships_create_many',
      type: body.type
    })
    return { message: `Relations have been successfully created` }
  }

  @Post('/search')
  @ApiBearerAuth()
  @UseGuards(HeavySearchLimitsGuard, IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UseInterceptors(TrackHeavySearchKu())
  @HttpCode(HttpStatus.OK)
  @TokenReadAccess()
  async findRelations(
    @PreferredTransactionDecorator() transaction: Transaction,
    @Body() searchQuery: RelationshipSearchDto = {},
    @Request() request: PlatformRequest
  ): Promise<TRecordRelationsResponse> {
    const projectId = request.projectId

    const skip = searchQuery.skip ?? 0
    const limit = searchQuery.limit ?? 1000

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

  @Post('/delete-many')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(createRelationsByKeysSchema, 'body'))
  @UseInterceptors(RunSideEffectMixin([ESideEffectType.RECALCULATE_SCHEMA_CACHE]))
  @HttpCode(HttpStatus.OK)
  async deleteMany(
    @Body()
    body: {
      source: { label: string; key?: string; where?: Where }
      target: { label: string; key?: string; where?: Where }
      type?: string
      direction?: TRelationDirection
      manyToMany?: boolean
    },
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<{ message: string }> {
    const projectId = request.projectId
    const normalizedDirection = body.direction?.toLowerCase() as TRelationDirection | undefined
    const deletedCount = await this.entityService.deleteRelationsByKeys({
      ...body,
      direction: normalizedDirection,
      projectId,
      transaction,
      manyToMany: body.manyToMany
    })
    this.trackRelationshipKu(request, KuOperation.KNOWLEDGE_DELETED, deletedCount, {
      kind: 'relationship',
      trigger: 'relationships_delete_many'
    })
    return { message: `Relations have been successfully deleted` }
  }
}
