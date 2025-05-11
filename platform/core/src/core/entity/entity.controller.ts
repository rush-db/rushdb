import {
  Body,
  Controller,
  DefaultValuePipe,
  Delete,
  Get,
  HttpCode,
  HttpStatus,
  Logger,
  NotFoundException,
  Param,
  ParseIntPipe,
  Patch,
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
import { ESideEffectType, RunSideEffectMixin } from '@/common/interceptors/run-side-effect.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { asyncAssertExistsOrThrow } from '@/common/utils/asyncAssertExistsOrThrow'
import { isDevMode } from '@/common/utils/isDevMode'
import { omit } from '@/common/utils/omit'
import { ValidationPipe } from '@/common/validation/validation.pipe'
import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_LABEL,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META
} from '@/core/common/constants'
import { prepareProperties } from '@/core/common/normalizeRecord'
import { LinkEntityDto } from '@/core/entity/dto/link-entity.dto'
import { UnlinkEntityDto } from '@/core/entity/dto/unlink-entity.dto'
import { EntityWriteGuard } from '@/core/entity/entity-write.guard'
import { TRecordRelationsResponse, TRecordSearchResult } from '@/core/entity/entity.types'
import { TEntityPropertiesNormalized } from '@/core/entity/model/entity.interface'
import { createEntitySchema } from '@/core/entity/validation/schemas/create-entity.schema'
import { editEntitySchema } from '@/core/entity/validation/schemas/edit-entity.schema'
import {
  createRelationSchema,
  deleteRelationsSchema
} from '@/core/entity/validation/schemas/relations.schema'
import { TPropertyProperties } from '@/core/property/model/property.interface'
import { PropertyService } from '@/core/property/property.service'
import { PropertyValuesPipe } from '@/core/property/validation/property-values.pipe'
import { SearchDto } from '@/core/search/dto/search.dto'
import { pagination } from '@/core/search/parser/pagination'
import { searchSchema } from '@/core/search/validation/schemas/search.schema'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { IsRelatedToProjectGuard } from '@/dashboard/auth/guards/is-related-to-project.guard'
import { PlanLimitsGuard } from '@/dashboard/billing/guards/plan-limits.guard'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaTransactionInterceptor } from '@/database/neogma/neogma-transaction.interceptor'

import { CreateEntityDto, CreateEntityDtoSimple } from './dto/create-entity.dto'
import { EditEntityDto } from './dto/edit-entity.dto'
import { EntityService } from './entity.service'
import { CustomTransactionInterceptor } from '@/database/neogma-dynamic/custom-transaction.interceptor'
import { PreferredTransactionDecorator } from '@/database/neogma-dynamic/preferred-transaction.decorator'
import { CustomDbWriteRestrictionGuard } from '@/dashboard/billing/guards/custom-db-write-restriction.guard'

// ---------------------------------------------------------------------------------------------------------------------
// RECORDS CRUD

// POST     /records                     ✅ CREATE
// GET      /records/:id                 ✅ READ
// PUT      /records/:id                 ✅ REWRITE
// DELETE   /records/:id                 ✅ DELETE

// RECORDS EXTRA

// POST     /records/:id/link            ✅ LINK
// POST     /records/:id/unlink          ✅ UNLINK

// POST     /records/:id/properties      ✅ FIELDS LIST (SEARCHABLE)
// POST     /records/:id/labels          ❌ LABELS LIST (SEARCHABLE) @TODO

// RECORDS SEARCH

// POST     /records/search              ✅ SEARCH SearchDto
// POST     /records/:id/search          ✅ SEARCH SearchDto

type BulkUpdateRecords = {
  where: SearchDto
  data: {
    name: null
    fullName: { alias?: '$record'; field: 'name' }
  }
}

// ---------------------------------------------------------------------------------------------------------------------

@Controller('records')
@ApiTags('Records')
@UseInterceptors(
  TransformResponseInterceptor,
  NotFoundInterceptor,
  NeogmaDataInterceptor,
  NeogmaTransactionInterceptor,
  CustomTransactionInterceptor
)
export class EntityController {
  constructor(
    private readonly entityService: EntityService,
    private readonly propertyService: PropertyService
  ) {}

  @Get(':entityId')
  @ApiParam({
    name: 'entityId',
    required: true,
    description: 'entity identifier (UUIDv7)',
    type: 'string'
  })
  @ApiBearerAuth()
  @AuthGuard()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  async getEntity(
    @Param('entityId') entityId: string,
    @PreferredTransactionDecorator() transaction: Transaction
  ): Promise<TEntityPropertiesNormalized> {
    return await this.entityService.getEntity({
      id: entityId,
      transaction
    })
  }

  @Post()
  @ApiBearerAuth()
  @UseGuards(PlanLimitsGuard, IsRelatedToProjectGuard(), EntityWriteGuard, CustomDbWriteRestrictionGuard)
  @UsePipes(ValidationPipe(createEntitySchema, 'body'), PropertyValuesPipe)
  @UseInterceptors(RunSideEffectMixin([ESideEffectType.RECOUNT_PROJECT_STRUCTURE]))
  @HttpCode(HttpStatus.CREATED)
  @AuthGuard('project')
  async createEntity(
    @Body() entity: CreateEntityDto | CreateEntityDtoSimple,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<TEntityPropertiesNormalized> {
    const projectId = request.projectId

    const result = await this.entityService.createEntity({
      entity,
      projectId,
      // we need smart switcher between customTx and default service tx, maybe new decorator
      transaction
    })

    await this.propertyService.deleteOrphanProps({
      projectId,
      transaction
    })

    return result
  }

  @Patch(':entityId')
  @ApiParam({
    name: 'entityId',
    required: true,
    description: 'entity identifier (UUIDv7)',
    type: 'string'
  })
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard(), EntityWriteGuard)
  @AuthGuard('project')
  @UsePipes(ValidationPipe(editEntitySchema, 'body'), PropertyValuesPipe)
  @UseInterceptors(RunSideEffectMixin([ESideEffectType.RECOUNT_PROJECT_STRUCTURE]))
  @HttpCode(HttpStatus.CREATED)
  async updateEntity(
    @Param('entityId') entityId: string,
    @Body() entity: EditEntityDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<TEntityPropertiesNormalized> {
    const projectId = request.projectId

    const currentRecordData = await asyncAssertExistsOrThrow(
      () => this.entityService.getEntity({ id: entityId, transaction }),
      new NotFoundException(`Record with id '${entityId}' not found`)
    )

    const ownProps = omit(currentRecordData, [
      RUSHDB_KEY_LABEL,
      RUSHDB_KEY_ID,
      RUSHDB_KEY_PROJECT_ID,
      RUSHDB_KEY_PROPERTIES_META
    ])

    const record = await this.entityService.editEntity({
      entityId,
      projectId,
      entity: {
        ...entity,
        properties: [...prepareProperties(ownProps), ...entity.properties]
      },
      transaction
    })

    await this.propertyService.deleteOrphanProps({
      projectId,
      transaction
    })

    return record
  }

  @Put(':entityId')
  @ApiParam({
    name: 'entityId',
    required: true,
    description: 'entity identifier (UUIDv7)',
    type: 'string'
  })
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard(), EntityWriteGuard)
  @AuthGuard('project')
  @UsePipes(ValidationPipe(editEntitySchema, 'body'), PropertyValuesPipe)
  @UseInterceptors(RunSideEffectMixin([ESideEffectType.RECOUNT_PROJECT_STRUCTURE]))
  @HttpCode(HttpStatus.CREATED)
  async setEntity(
    @Param('entityId') entityId: string,
    @Body() entity: EditEntityDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<TEntityPropertiesNormalized> {
    const projectId = request.projectId

    const record = await this.entityService.editEntity({
      entityId,
      projectId,
      entity,
      transaction
    })

    await this.propertyService.deleteOrphanProps({
      projectId,
      transaction
    })

    return record
  }

  @Put('delete')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UseInterceptors(RunSideEffectMixin([ESideEffectType.RECOUNT_PROJECT_STRUCTURE]))
  async deleteBulk(
    @PreferredTransactionDecorator() transaction: Transaction,
    @Body() searchParams: SearchDto = {},
    @Request() request: PlatformRequest
  ): Promise<{ message: string }> {
    const projectId = request.projectId

    return await this.entityService.deleteRecords({
      projectId,
      searchParams,
      transaction
    })
  }

  @Delete(':entityId')
  @ApiParam({
    name: 'entityId',
    required: true,
    description: 'entity identifier (UUIDv7)',
    type: 'string'
  })
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UseInterceptors(RunSideEffectMixin([ESideEffectType.RECOUNT_PROJECT_STRUCTURE]))
  async deleteEntity(
    @Param('entityId') entityId: string,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<{ message: string }> {
    const projectId = request.projectId
    return await this.entityService.deleteEntity(entityId, projectId, transaction)
  }

  @Post('/search')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async searchFromRoot(
    @PreferredTransactionDecorator() transaction: Transaction,
    @Body() searchParams: SearchDto,
    @Request() request: PlatformRequest
  ): Promise<TRecordSearchResult> {
    const projectId = request.projectId
    try {
      const [data, total] = await Promise.all([
        this.entityService.findRecords({
          projectId,
          searchParams,
          transaction
        }),
        this.entityService.getRecordsTotalCount({
          projectId,
          searchParams,
          transaction
        })
      ])
      return {
        data,
        total
      }
    } catch (e) {
      isDevMode(() => Logger.error(e))
    }
  }

  @Post(':entityId/search')
  @ApiParam({
    name: 'entityId',
    required: true,
    description: 'entity identifier (UUIDv7)',
    type: 'string'
  })
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async levelSearch(
    @Param('entityId') entityId: string,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Body() searchParams: SearchDto = {},
    @Request() request: PlatformRequest
  ): Promise<TRecordSearchResult> {
    const projectId = request.projectId

    const [data, total] = await Promise.all([
      this.entityService.findRecords({
        id: entityId,
        projectId,
        searchParams,
        transaction
      }),
      this.entityService.getRecordsTotalCount({
        id: entityId,
        projectId,
        searchParams,
        transaction
      })
    ])

    return {
      data,
      total
    }
  }

  @Get(':entityId/properties')
  @ApiParam({
    name: 'entityId',
    required: true,
    description: 'entity identifier (UUIDv7)',
    type: 'string'
  })
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async getEntityFields(
    @Param('entityId') entityId: string,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<{ data: TPropertyProperties[] }> {
    const projectId = request.projectId

    const data = await this.entityService.getEntityFields({
      id: entityId,
      projectId,
      transaction
    })
    return { data }
  }

  @Get(':entityId/relations')
  @ApiParam({
    name: 'entityId',
    required: true,
    description: 'entity identifier (UUIDv7)',
    type: 'string'
  })
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async getRecordRelations(
    @Param('entityId') entityId: string,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest,
    @Query('skip', new DefaultValuePipe(0), ParseIntPipe) skip?: number,
    @Query('limit', new DefaultValuePipe(1000), ParseIntPipe) limit?: number
  ): Promise<TRecordRelationsResponse> {
    const projectId = request.projectId
    const [data, total] = await Promise.all([
      this.entityService.getRecordRelations({
        id: entityId,
        searchParams: {},
        pagination: pagination(skip, limit),
        projectId,
        transaction
      }),
      this.entityService.getRecordRelationsCount({
        id: entityId,
        searchParams: {},
        projectId,
        transaction
      })
    ])

    return {
      data,
      total
    }
  }

  @Post(':entityId/relations')
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
    }),
    CustomDbWriteRestrictionGuard
  )
  @UsePipes(ValidationPipe(createRelationSchema, 'body'))
  @AuthGuard('project')
  @HttpCode(HttpStatus.CREATED)
  async linkEntity(
    @Param('entityId') entityId: string,
    @Body() linkEntity: LinkEntityDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<{ message: string }> {
    const projectId = request.projectId
    return await this.entityService.attachRecords(entityId, linkEntity, projectId, transaction)
  }

  @Put(':entityId/relations')
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
    }),
    CustomDbWriteRestrictionGuard
  )
  @UsePipes(ValidationPipe(deleteRelationsSchema, 'body'))
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async deleteRecordRelations(
    @Param('entityId') entityId: string,
    @Body() unlinkEntityDto: UnlinkEntityDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<{ message: string }> {
    const projectId = request.projectId
    return await this.entityService.detachRecords(entityId, unlinkEntityDto, projectId, transaction)
  }

  @Post('relations/search')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async searchRecordRelations(
    @PreferredTransactionDecorator() transaction: Transaction,
    @Body() searchParams: SearchDto = {},
    @Request() request: PlatformRequest,
    @Query('skip', new DefaultValuePipe(0), ParseIntPipe) skip?: number,
    @Query('limit', new DefaultValuePipe(1000), ParseIntPipe) limit?: number
  ): Promise<TRecordRelationsResponse> {
    const projectId = request.projectId

    const [data, total] = await Promise.all([
      this.entityService.getRecordRelations({
        searchParams,
        pagination: pagination(skip, limit),
        projectId,
        transaction
      }),
      this.entityService.getRecordRelationsCount({
        projectId,
        searchParams,
        transaction
      })
    ])

    return {
      data,
      total
    }
  }
}
