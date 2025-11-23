import {
  Body,
  Controller,
  Delete,
  Get,
  HttpCode,
  HttpStatus,
  Logger,
  NotFoundException,
  Param,
  Patch,
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
import { asyncAssertExistsOrThrow } from '@/common/utils/asyncAssertExistsOrThrow'
import { isArray } from '@/common/utils/isArray'
import { isDevMode } from '@/common/utils/isDevMode'
import { omit } from '@/common/utils/omit'
import { toBoolean } from '@/common/utils/toBolean'
import { ValidationPipe } from '@/common/validation/validation.pipe'
import {
  RUSHDB_KEY_ID,
  RUSHDB_KEY_LABEL,
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META
} from '@/core/common/constants'
import { prepareProperties } from '@/core/common/normalizeRecord'
import { EntityWriteGuard } from '@/core/entity/entity-write.guard'
import { TEntityPropertiesNormalized, TRecordSearchResult } from '@/core/entity/entity.types'
import { createEntitySchema } from '@/core/entity/validation/schemas/create-entity.schema'
import { editEntitySchema } from '@/core/entity/validation/schemas/edit-entity.schema'
import { PropertyService } from '@/core/property/property.service'
import { PropertyValuesPipe } from '@/core/property/validation/property-values.pipe'
import { SearchDto } from '@/core/search/dto/search.dto'
import { searchSchema } from '@/core/search/validation/schemas/search.schema'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { IsRelatedToProjectGuard } from '@/dashboard/auth/guards/is-related-to-project.guard'
import { CustomDbWriteRestrictionGuard } from '@/dashboard/billing/guards/custom-db-write-restriction.guard'
import { PlanLimitsGuard } from '@/dashboard/billing/guards/plan-limits.guard'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'
import { PreferredTransactionDecorator } from '@/database/preferred-transaction.decorator'

import { CreateEntityDto } from './dto/create-entity.dto'
import { EditEntityDto } from './dto/edit-entity.dto'
import { EntityService } from './entity.service'

@Controller('records')
@ApiTags('Records')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, DataInterceptor)
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
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  async getById(
    @Param('entityId') entityId: string,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<TEntityPropertiesNormalized> {
    const projectId = request.projectId

    return await this.entityService.getById({
      id: entityId,
      projectId,
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
  async create(
    @Body() entity: CreateEntityDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<TEntityPropertiesNormalized> {
    const projectId = request.projectId
    const wantsUpsert = toBoolean(entity.options?.mergeStrategy) || isArray(entity.options?.mergeBy)

    if (wantsUpsert) {
      return await this.entityService.upsert({
        entity,
        projectId,
        transaction
      })
    }

    const result = await this.entityService.create({
      entity,
      projectId,
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
  async update(
    @Param('entityId') entityId: string,
    @Body() entity: EditEntityDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<TEntityPropertiesNormalized> {
    const projectId = request.projectId

    const currentRecordData = await asyncAssertExistsOrThrow(
      () => this.entityService.getById({ id: entityId, projectId, transaction }),
      new NotFoundException(`Record with id '${entityId}' not found`)
    )

    const currentOwnProperties = omit(currentRecordData, [
      RUSHDB_KEY_LABEL,
      RUSHDB_KEY_ID,
      RUSHDB_KEY_PROJECT_ID,
      RUSHDB_KEY_PROPERTIES_META
    ])

    const normalizedOwnProperties = prepareProperties(currentOwnProperties)

    const record = await this.entityService.edit({
      entityId,
      projectId,
      entity: {
        ...entity,
        properties: [...normalizedOwnProperties, ...entity.properties]
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
  async set(
    @Param('entityId') entityId: string,
    @Body() entity: EditEntityDto,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<TEntityPropertiesNormalized> {
    const projectId = request.projectId

    // let entityDraft: EditEntityDto
    //
    // if ('properties' in entity) {
    //   entityDraft = entity
    // }
    //
    // if ('data' in entity) {
    //   // @TODO: Implement schema schema validation https://github.com/rush-db/rushdb/issues/43
    //   entityDraft = {
    //     label: entity.label,
    //     properties: [...normalizeRecord(entity).properties]
    //   }
    // }

    const record = await this.entityService.edit({
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

  @Post('delete')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UseInterceptors(RunSideEffectMixin([ESideEffectType.RECOUNT_PROJECT_STRUCTURE]))
  async delete(
    @PreferredTransactionDecorator() transaction: Transaction,
    @Body() searchQuery: SearchDto = {},
    @Request() request: PlatformRequest
  ): Promise<{ message: string }> {
    const projectId = request.projectId

    return await this.entityService.delete({
      projectId,
      searchQuery,
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
  async deleteById(
    @Param('entityId') entityId: string,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<{ message: string }> {
    const projectId = request.projectId
    return await this.entityService.deleteById({ id: entityId, projectId, transaction })
  }

  @Post('/search')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async find(
    @PreferredTransactionDecorator() transaction: Transaction,
    @Body() searchQuery: SearchDto,
    @Request() request: PlatformRequest
  ): Promise<TRecordSearchResult> {
    const projectId = request.projectId
    try {
      const [data, total] = await Promise.all([
        this.entityService.find({
          projectId,
          searchQuery,
          transaction
        }),
        this.entityService.getCount({
          projectId,
          searchQuery,
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
  async findFromId(
    @Param('entityId') entityId: string,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Body() searchQuery: SearchDto = {},
    @Request() request: PlatformRequest
  ): Promise<TRecordSearchResult> {
    const projectId = request.projectId

    const [data, total] = await Promise.all([
      this.entityService.find({
        id: entityId,
        projectId,
        searchQuery,
        transaction
      }),
      this.entityService.getCount({
        id: entityId,
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
