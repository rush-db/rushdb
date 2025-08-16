import {
  Body,
  Controller,
  Delete,
  forwardRef,
  Get,
  HttpCode,
  HttpStatus,
  Inject,
  Param,
  Post,
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
import { TPropertyProperties } from '@/core/property/model/property.interface'
import { PropertyService } from '@/core/property/property.service'
import { SearchDto } from '@/core/search/dto/search.dto'
import { TSearchSortDirection } from '@/core/search/search.types'
import { searchSchema } from '@/core/search/validation/schemas/search.schema'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { IsRelatedToProjectGuard } from '@/dashboard/auth/guards/is-related-to-project.guard'
import { CustomDbWriteRestrictionGuard } from '@/dashboard/billing/guards/custom-db-write-restriction.guard'
import { NeogmaDataInterceptor } from '@/database/neogma/neogma-data.interceptor'
import { PreferredTransactionDecorator } from '@/database/neogma-dynamic/preferred-transaction.decorator'

@Controller('properties')
@ApiTags('Properties')
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, NeogmaDataInterceptor)
export class PropertyController {
  constructor(
    private readonly propertyService: PropertyService,
    @Inject(forwardRef(() => EntityService))
    private readonly entityService: EntityService
  ) {}

  @Post('/search')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard())
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async listProperties(
    @Body() searchQuery: Omit<SearchDto, 'sort' | 'skip' | 'limit'>,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<TPropertyProperties[]> {
    const projectId = request.projectId

    return this.entityService.findProperties({
      projectId,
      searchQuery,
      transaction
    })
  }

  @Post(':propertyId/values')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard(), CustomDbWriteRestrictionGuard)
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async getPropertyValues(
    @Param('propertyId') propertyId: string,
    @Body() searchQueryWithQuery: SearchDto & { query?: string; orderBy?: TSearchSortDirection },
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<unknown> {
    const projectId = request.projectId

    return await this.propertyService.getPropertyValues({
      propertyId,
      searchQuery: searchQueryWithQuery,
      projectId,
      transaction
    })
  }

  @Get(':propertyId')
  @ApiBearerAuth()
  @UseGuards(IsRelatedToProjectGuard(), CustomDbWriteRestrictionGuard)
  @AuthGuard('project')
  @UsePipes(ValidationPipe(searchSchema, 'body'))
  @HttpCode(HttpStatus.OK)
  async findById(
    @Param('propertyId') propertyId: string,
    @PreferredTransactionDecorator() transaction: Transaction,
    @Request() request: PlatformRequest
  ): Promise<Omit<TPropertyProperties, 'projectId'>> {
    const projectId = request.projectId

    return await this.propertyService.findById({
      propertyId,
      projectId,
      transaction
    })
  }

  // @TODO: Make it with POST :id/delete (SearchDTO => delete Property for selected Records)

  @Delete(':propertyId')
  @ApiBearerAuth()
  @UseGuards(
    IsRelatedToProjectGuard(),
    // @TODO: Andrew? help
    CustomDbWriteRestrictionGuard
  )
  @AuthGuard('project')
  @HttpCode(HttpStatus.OK)
  async deleteField(
    @Param('propertyId') propertyId: string,
    // @TODO: Revamp it with put/post to meed HTTP Spec (no body in DELETE)
    // @Body() deleteParams: DeletePropertyDto,
    @PreferredTransactionDecorator() transaction: Transaction,
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
  // @Patch(':propertyId/values')
  // @ApiBearerAuth()
  // @UseGuards(IsRelatedToProjectGuard(), CustomDbWriteRestrictionGuard)
  // @AuthGuard('project')
  // @HttpCode(HttpStatus.OK)
  // async updateFieldValues(
  //   @Param('propertyId') propertyId: string,
  //   @Body() updateParams: UpdatePropertyValueDto,
  //   @PreferredTransactionDecorator() transaction: Transaction,
  //   @Request() request: PlatformRequest
  // ): Promise<boolean> {
  //   const projectId = request.projectId
  //
  //   if (updateParams.newValue === null) {
  //     const deleteParams = {
  //       entityIds: updateParams.entityIds
  //     }
  //
  //     await this.propertyService.deleteProperty({
  //       propertyId,
  //       deleteParams,
  //       projectId,
  //       transaction
  //     })
  //
  //     return true
  //   }
  //
  //   await this.propertyService.updateField({
  //     propertyId,
  //     updateParams,
  //     transaction
  //   })
  //   return true
  // }

  // @TODO: Rename operation here is ok too. But maybe it would be better to achieve the same more flexible
  // by putting Rename method as a part of Bulk Operation to Entity Scope. It will be cool to have SearchDTO as
  // a part this API.
  // @Patch(':propertyId')
  // @ApiBearerAuth()
  // @UseGuards(IsRelatedToProjectGuard(), CustomDbWriteRestrictionGuard)
  // @AuthGuard('project')
  // @HttpCode(HttpStatus.OK)
  // async updateField(
  //   @Param('propertyId') propertyId: string,
  //   @Body() updateParams: UpdatePropertyDto,
  //   @PreferredTransactionDecorator() transaction: Transaction,
  //   @Request() request: PlatformRequest
  // ): Promise<boolean> {
  //   const projectId = request.projectId
  //
  //   await this.propertyService.updateFieldData({
  //     propertyId,
  //     updateParams,
  //     transaction,
  //     projectId
  //   })
  //
  //   return true
  // }
}
