import {
  Body,
  Controller,
  DefaultValuePipe,
  Delete,
  forwardRef,
  Get,
  HttpCode,
  HttpStatus,
  Inject,
  Param,
  ParseIntPipe,
  Patch,
  Post,
  Query,
  Request,
  UseGuards,
  UseInterceptors,
  UsePipes
} from '@nestjs/common'
import { ApiBearerAuth, ApiTags } from '@nestjs/swagger'
import { Transaction } from 'neo4j-driver'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { ValidationPipe } from '@/common/validation/validation.pipe'
import { EntityService } from '@/core/entity/entity.service'
import { UpdatePropertyValueDto } from '@/core/property/dto/update-property-value.dto'
import { UpdatePropertyDto } from '@/core/property/dto/update-property.dto'
import { TPropertyProperties } from '@/core/property/model/property.interface'
import { PropertyService } from '@/core/property/property.service'
import { SearchDto } from '@/core/search/dto/search.dto'
import { pagination } from '@/core/search/parser/pagination'
import { TSearchSortDirection } from '@/core/search/search.types'
import { searchSchema } from '@/core/search/validation/schemas/search.schema'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { IsRelatedToProjectGuard } from '@/dashboard/auth/guards/is-related-to-project.guard'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { NeogmaTransactionInterceptor } from '@/database/neogma/neogma-transaction.interceptor'
import { TransactionDecorator } from '@/database/neogma/transaction.decorator'

// FIELDS CRUD

// POST     /field                      ✅ SEARCH Fields SearchDto

// GET      /field                      ✅ READ ALL
// GET      /field/:id                  ✅ READ
// PATCH    /field/:id                  ✅ UPDATE DTO?: { name?: string, type?: 'string' }
// PATCH    /field/:id/values           ✅ UPDATE DTO?: { newValue?: undefined | ... } & { SearchDTO | { entityIds?: string[] | "*", depth: number = 1 | full } }
// DELETE   /field/:id                  ✅ DELETE DTO?: { SearchDTO // 2nd stage | { entityIds?: string[] | "*", depth: number = 0 | full } // 1st stage }

// type TUpdateProp | TCopyProp = {
//     id?: string;
//     target: SearchDto | { entityIds?: string[] | '*'; depth: TSearchDepth };
//     name?: string;
//     newValue?: TPropertyValueRaw;
//     valueMatcher: 'value - {OPERATION} - critera';
// };

// 1. Don't know how to merge similar properties (if name and type are matched)
// 2. Don't know how to change prop type // we are ok with it because thinking of it causes pain in the ass
// 3. Move prop values partially to new/existing prop (for normalization purposes) (Copying, Backuping)

@Controller('properties')
@ApiTags('Properties')
@UseInterceptors(
  TransformResponseInterceptor,
  NotFoundInterceptor,
  NeogmaDataInterceptor,
  NeogmaTransactionInterceptor
)
export class PropertyController {
  constructor(
    private readonly propertyService: PropertyService,
    @Inject(forwardRef(() => EntityService))
    private readonly entityService: EntityService
  ) {}

  @Post()
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async listProperties(
    @Body() searchParams: Omit<SearchDto, 'sort' | 'skip' | 'limit'>,
    @TransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<TPropertyProperties[]> {
    const projectId = request.projectId

    return this.entityService.getEntityFields({
      projectId,
      searchParams,
      transaction
    })
  }

  @Get(':propertyId/values')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async getPropertyValues(
    @Param('propertyId') propertyId: string,
    @TransactionDecorator() transaction: Transaction,
    @Query('sort') sort?: TSearchSortDirection,
    @Query('query') query?: string,
    @Query('skip', new DefaultValuePipe(0), ParseIntPipe) skip?: number,
    @Query('limit', new DefaultValuePipe(100), ParseIntPipe) limit?: number
  ): Promise<unknown> {
    try {
      return await this.propertyService.getPropertyValues({
        propertyId,
        sort,
        query,
        pagination: pagination(skip, limit),
        transaction
      })
    } catch (error) {
      console.log(error)
    }
  }

  @Get(':propertyId')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async findById(
    @Param('propertyId') propertyId: string,
    @TransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<unknown> {
    const projectId = request.projectId

    return await this.propertyService.findById({
      propertyId,
      projectId,
      transaction
    })
  }

  // @TODO: Move it to Entity Scope Bulk Operations. (SearchDTO => delete Property for selected Records)
  // It will become more useful when posed as Record-centric API.

  @Delete(':propertyId')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async deleteField(
    @Param('propertyId') propertyId: string,
    // @TODO: Revamp it with put/post to meed HTTP Spec (no body in DELETE)
    // @Body() deleteParams: DeletePropertyDto,
    @TransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<{ message: string }> {
    const projectId = request.projectId
    await this.propertyService.deleteProperty({
      propertyId,
      // deleteParams,
      projectId,
      transaction
    })

    return { message: `Property (${propertyId}) has been successfully deleted.` }
  }

  // @TODO: Move to bulk patch operation to Entity Scope
  @Patch(':propertyId/values')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async updateFieldValues(
    @Param('propertyId') propertyId: string,
    @Body() updateParams: UpdatePropertyValueDto,
    @TransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<boolean> {
    const projectId = request.projectId

    if (updateParams.newValue === null) {
      const deleteParams = {
        entityIds: updateParams.entityIds
      }

      await this.propertyService.deleteProperty({
        propertyId,
        deleteParams,
        projectId,
        transaction
      })

      return true
    }

    await this.propertyService.updateField({
      propertyId,
      updateParams,
      transaction
    })
    return true
  }

  // @TODO: Rename operation here is ok too. But maybe it would be better to achieve the same more flexible
  // by putting Rename method as a part of Bulk Operation to Entity Scope. It will be cool to have SearchDTO as
  // a part this API.
  @Patch(':propertyId')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async updateField(
    @Param('propertyId') propertyId: string,
    @Body() updateParams: UpdatePropertyDto,
    @TransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<boolean> {
    const projectId = request.projectId

    await this.propertyService.updateFieldData({
      propertyId,
      updateParams,
      transaction,
      projectId
    })

    return true
  }
}
